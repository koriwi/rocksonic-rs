pub mod libs;
use crate::libs::{
    ffmpeg, magick, responses::SubSonicSong, server::Server, sync, utils::download_file,
};

use dirs::home_dir;
use magick_rust::magick_wand_genesis;
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{self, AtomicU32},
    thread::sleep,
    time::Duration,
};

use anyhow::{anyhow, Context, Error, Result};
use clap::Parser;
use colored::Colorize;
use rayon::prelude::*;

#[derive(Debug)]
enum Action {
    Downloaded,
    CoverDownloaded,
    CoverConverted,
    Converted,
    CoverEmbedded,
}

fn setup_dirs(rocksonic_dir: &str, library_dir: &str, cache_dir: &str) -> Result<()> {
    fs::create_dir_all(format!("{}/{}", rocksonic_dir, cache_dir))?;
    fs::create_dir_all(format!("{}/.cover", rocksonic_dir))?;
    fs::create_dir_all(format!("{}/{}", rocksonic_dir, library_dir))?;
    Ok(())
}

fn process_song(
    song: &SubSonicSong,
    server: &Server,
    rocksonic_dir: &str,
    cache_dir: &str,
    output_dir: &str,
    library_dir: &str,
    mp3: Option<u16>,
    flat: bool,
    coversize: u16,
) -> Result<Vec<Action>> {
    let mut actions = vec![];

    let cache_variant = mp3
        .map(|bitrate| format!("mp3_{bitrate}"))
        .unwrap_or_else(|| String::from("orig"));
    let cached_path = format!(
        "{}/{}/{}_{}",
        rocksonic_dir, cache_dir, song.id, cache_variant
    );
    let song_downloaded = !fs::exists(&cached_path)?;
    if song_downloaded {
        actions.push(Action::Downloaded);
        let mut song_res = server.get_song(&song.id, mp3)?;
        download_file(&mut song_res, &cached_path)?;
    }

    let cover_path = format!("{}/.cover/{}_{}", rocksonic_dir, song.id, coversize);
    if !fs::exists(&cover_path)? {
        let mut cover_response = server.get_cover_art(&song.id, coversize)?;
        download_file(&mut cover_response, &cover_path)?;
        actions.push(Action::CoverDownloaded);
    }

    let converted_cover_path = format!(
        "{}/.cover/{}_{}_baseline",
        rocksonic_dir, song.id, coversize
    );
    if !fs::exists(&converted_cover_path)? {
        magick::convert_image(&cover_path, &converted_cover_path)?;
        actions.push(Action::CoverConverted);
    }

    let output_path = sync::song_output_path(song, output_dir, library_dir, mp3, flat);

    if !flat {
        let album_dir = Path::new(&output_path).parent().unwrap().to_str().unwrap();
        if !fs::exists(album_dir)? {
            fs::create_dir_all(album_dir)?;
            let cover_art_path = format!("{}/cover.jpeg", album_dir);
            fs::copy(&converted_cover_path, &cover_art_path)?;
        }
    }

    if song_downloaded || !fs::exists(&output_path)? {
        ffmpeg::combine_song_and_cover(&cached_path, &converted_cover_path, &output_path)?;
        actions.push(Action::CoverEmbedded);
        if mp3.is_some() {
            actions.push(Action::Converted);
        }
    }

    Ok(actions)
}

#[derive(Parser, Debug)]
#[command(disable_help_flag = true, long_about = None, ignore_errors = true)]
struct DaemonArg {
    #[arg(short, long)]
    daemon: Option<PathBuf>,

    #[arg(long, value_name = "FILE", conflicts_with = "daemon")]
    config: Option<PathBuf>,
}

#[derive(Parser, Debug, Default, Deserialize)]
#[command(version, about, long_about = None, disable_help_flag = true)]
struct Args {
    #[arg()]
    output_dir: Option<String>,

    #[arg(
        short,
        long,
        help = "Don't forget the \"/rest\"",
        required = false,
        required_unless_present = "config"
    )]
    host: String,

    #[arg(short, long, required = false, required_unless_present = "config")]
    username: String,

    #[arg(short, long, required = false, required_unless_present = "config")]
    password: String,

    #[arg(long, action = clap::ArgAction::Help)]
    help: Option<bool>,

    #[arg(short, long, help = "enables mp3 conversion. parameter in kbits")]
    mp3: Option<u16>,

    #[serde(default = "default_coversize")]
    #[arg(short, long, default_value_t = default_coversize())]
    coversize: u16,

    #[arg(short, long)]
    threads: Option<u16>,

    #[serde(default)]
    #[arg(
        short,
        long,
        help = "put all files in one folder. puts the artist and album name in the file name"
    )]
    flat: bool,

    #[arg(short = 'l', long, help = "use fav (liked songs) or the <playlist-id>")]
    playlist: Option<String>,

    #[serde(default)]
    #[arg(
        short,
        long,
        help = "remove files from the output folder that are no longer in the playlist"
    )]
    sync: bool,

    #[serde(skip)]
    #[arg(
        long,
        value_name = "FILE",
        help = "read options from a JSON config file"
    )]
    config: Option<PathBuf>,
}

fn default_coversize() -> u16 {
    500
}

fn load_config(path: &Path) -> Result<Args> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("could not read config file {}", path.display()))?;
    let mut args: Args = serde_json::from_str(&content)
        .with_context(|| format!("config file {} is not valid JSON", path.display()))?;
    resolve_config_output_dir(&mut args, path);
    Ok(args)
}

fn resolve_config_output_dir(args: &mut Args, config_path: &Path) {
    let Some(output_dir) = args.output_dir.as_ref() else {
        return;
    };
    if Path::new(output_dir).is_absolute() {
        return;
    }

    let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
    args.output_dir = Some(config_dir.join(output_dir).to_string_lossy().into_owned());
}

fn parse_args(daemon_arg: Option<&Path>, config: Option<&Path>) -> Result<Args> {
    if daemon_arg.is_some() && config.is_some() {
        return Err(anyhow!("--config cannot be used with --daemon"));
    }
    if let Some(config) = config {
        return load_config(config);
    }

    let Some(daemon_dir) = daemon_arg else {
        return Ok(Args::parse());
    };
    println!("searching for device with rocksonic.json");
    loop {
        sleep(Duration::from_millis(500));
        let dirs = fs::read_dir(daemon_dir).expect("could not read daemon dir");
        let found_file = dirs
            .filter_map(|dir_result| dir_result.ok())
            .find_map(|dir| {
                fs::read_dir(dir.path()).unwrap().find_map(|entry| {
                    let file = entry.ok()?;
                    if file.file_name() == "rocksonic.json" {
                        Some((dir.path(), file))
                    } else {
                        None
                    }
                })
            });
        if let Some((dir, file)) = found_file {
            let mut args = load_config(&file.path())?;
            args.output_dir = Some(String::from(dir.to_str().unwrap()));
            break Ok(args);
        }
    }
}

fn main() -> Result<()> {
    magick_wand_genesis();
    let rocksonic_dir = home_dir().expect("could not find home directory");
    let rocksonic_dir = format!(
        "{}/.local/share/rocksonic_songs",
        rocksonic_dir.to_str().expect("invalid unicode path")
    );

    let daemon_arg = DaemonArg::parse();

    loop {
        let args = parse_args(daemon_arg.daemon.as_deref(), daemon_arg.config.as_deref())?;
        let output_dir = args.output_dir.clone().unwrap_or(String::from(
            std::env::current_dir().unwrap().to_str().unwrap(),
        ));

        let server =
            Server::connect(args.host, args.username, args.password).inspect_err(|_e| {
                println!("Could not connect to the server. Did you forget /rest ?");
            })?;

        let mut library_dir = match args.playlist.as_ref() {
            None => String::from("favs"),
            Some(playlist) => server.get_playlist(playlist)?.playlist.name,
        };
        if args.flat {
            library_dir.push_str(" flat");
        }
        if args.mp3.is_some() {
            library_dir.push_str(" mp3");
        }
        let cache_dir = if args.mp3.is_some() { ".mp3" } else { ".cache" };
        setup_dirs(&rocksonic_dir, &library_dir, cache_dir)?;

        println!("Welcome to {}!", "RockSonic".yellow().bold());
        println!("{}", "Successfully connected to SubSonic".green().italic());

        let num_threads = args.threads.unwrap_or(5);
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads as usize)
            .build()?;
        let songs = match args.playlist.as_ref() {
            None => server.get_favs()?,
            Some(playlist_id) => server.get_playlist(playlist_id)?.playlist.songs,
        };
        let title_width = songs.iter().map(|s| s.title.len()).max().unwrap_or(0);

        let songs_done_counter = atomic::AtomicU32::new(0);
        pool.install(|| {
            songs
                .par_iter()
                .map(|song| -> Result<(&SubSonicSong, Vec<Action>)> {
                    process_song(
                        song,
                        &server,
                        &rocksonic_dir,
                        cache_dir,
                        &output_dir,
                        &library_dir,
                        args.mp3,
                        args.flat,
                        args.coversize,
                    )
                    .map(|actions| (song, actions))
                    .map_err(|e: Error| anyhow!("{} {} {}", song.title, song.id, e))
                })
                .for_each(|result| {
                    print_status(result, &songs_done_counter, songs.len(), title_width)
                });
        });

        if args.sync {
            let expected_paths: HashSet<PathBuf> = songs
                .iter()
                .map(|song| {
                    PathBuf::from(sync::song_output_path(
                        song,
                        &output_dir,
                        &library_dir,
                        args.mp3,
                        args.flat,
                    ))
                })
                .collect();
            sync::remove_unlisted_songs(&output_dir, &library_dir, &expected_paths, args.flat)?;
        }

        if daemon_arg.daemon.is_none() {
            break Ok(());
        }
        println!("daemon done!");
        println!("waiting for device to disconnect ...");
        while fs::exists(&output_dir).unwrap() {
            sleep(Duration::from_millis(100));
        }
        println!("device disconnected, awaiting new device ...");
    }
}

fn print_status(
    result: Result<(&SubSonicSong, Vec<Action>)>,
    counter: &AtomicU32,
    total: usize,
    title_width: usize,
) {
    let count = counter.fetch_add(1, atomic::Ordering::Release);
    match result {
        Ok((song, actions)) => {
            let actions_string = if actions.is_empty() {
                String::from("nothing to do")
            } else {
                actions
                    .iter()
                    .map(|action| format!("{:?}", action))
                    .collect::<Vec<String>>()
                    .join(", ")
            };
            println!(
                "{:-6}/{} {: ^width$} {}",
                count + 1,
                total,
                song.title,
                actions_string,
                width = title_width
            )
        }
        Err(e) => {
            println!("{:-6}/{} {:?}", count + 1, total, e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_uses_cli_defaults() {
        let args: Args = serde_json::from_str(
            r#"{
                "host": "https://music.example.com/rest",
                "username": "alice",
                "password": "secret"
            }"#,
        )
        .unwrap();

        assert_eq!(args.coversize, 500);
        assert!(!args.flat);
        assert!(!args.sync);
    }

    #[test]
    fn relative_output_dir_is_resolved_from_config_directory() {
        let mut args: Args = serde_json::from_str(
            r#"{
                "host": "https://music.example.com/rest",
                "username": "alice",
                "password": "secret",
                "output_dir": "../Music"
            }"#,
        )
        .unwrap();

        resolve_config_output_dir(&mut args, Path::new("/home/alice/.config/rocksonic.json"));

        assert_eq!(
            args.output_dir.as_deref(),
            Some("/home/alice/.config/../Music")
        );
    }
}
