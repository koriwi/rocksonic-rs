pub mod libs;
use crate::libs::{ffmpeg, magick, responses::SubSonicSong, server::Server, utils::download_file};

use dirs::home_dir;
use magick_rust::magick_wand_genesis;
use serde::Deserialize;
use std::{
    ffi::OsStr,
    fs,
    sync::atomic::{self, AtomicU32},
    thread::sleep,
    time::Duration,
};

use anyhow::{anyhow, Error, Result};
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

fn setup_dirs(rocksonic_dir: &str, library_dir: &str) -> Result<()> {
    let rs_dir = String::from(rocksonic_dir);
    println!("rs_dir {}", rs_dir);
    fs::create_dir_all(rs_dir.clone() + "/.mp3")?;
    fs::create_dir_all(rs_dir.clone() + "/.cover")?;
    fs::create_dir_all(rs_dir.clone() + library_dir)?;
    Ok(())
}

#[derive(Parser, Debug)]
#[command(disable_help_flag = true, long_about = None, ignore_errors = true)]
struct DaemonArg {
    #[arg(short, long)]
    daemon: Option<String>,
}

#[derive(Parser, Debug, Default, Deserialize)]
#[command(version, about, long_about = None, disable_help_flag = true)]
struct Args {
    #[arg()]
    output_dir: Option<String>,

    #[arg(short, long, help = "Don't forget the \"/rest\"")]
    host: String,

    #[arg(short, long)]
    username: String,

    #[arg(short, long)]
    password: String,

    #[arg(long, action = clap::ArgAction::Help)]
    help: Option<bool>,

    #[arg(short, long, help = "enables mp3 conversion. parameter in kbits")]
    mp3: Option<u16>,

    #[arg(short, long, default_value = "500")]
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
}

fn parse_args(daemon_arg: Option<String>) -> Args {
    if let Some(daemon_dir) = daemon_arg {
        println!("searching for device with rocksonic.json");
        loop {
            sleep(Duration::from_millis(500));
            let dirs = fs::read_dir(&daemon_dir).expect("could not read daemon dir");
            let found_file = dirs
                .filter_map(|dir_result| dir_result.ok())
                .find_map(|dir| {
                    fs::read_dir(dir.path()).unwrap().find_map(|dir_entry| {
                        if let Ok(file) = dir_entry {
                            if file.file_name() == "rocksonic.json" {
                                return Some((dir.path(), file));
                            }
                        }
                        None
                    })
                });
            if let Some((dir, file)) = found_file {
                let content = fs::read_to_string(file.path()).unwrap();
                let mut args = serde_json::from_str::<Args>(&content).expect("json file not valid");
                args.output_dir = Some(String::from(dir.to_str().unwrap()));
                break args;
            }
        }
    } else {
        Args::parse()
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
        let args: Args = parse_args(daemon_arg.daemon.clone());
        let output_dir = args.output_dir.clone().unwrap_or(String::from(
            std::env::current_dir().unwrap().to_str().unwrap(),
        ));

        let server =
            Server::connect(args.host, args.username, args.password).inspect_err(|_e| {
                println!("Could not connect to the server. Did you forget /rest ?");
            })?;

        let mut library_dir = String::from("/");
        library_dir += match args.playlist.as_ref() {
            None => String::from("favs"),
            Some(playlist) => server.get_playlist(playlist)?.playlist.name,
        }
        .as_str();
        if args.flat {
            library_dir = format!("{} flat", library_dir);
        }
        if args.mp3.is_some() {
            library_dir = format!("{} mp3", library_dir);
        }
        setup_dirs(&rocksonic_dir, &library_dir)?;

        println!("Welcome to {}!", "RockSonic".yellow().bold());
        println!("{}", "Successfully connected to SubSonic".green().italic());

        let num_threads = args.threads.unwrap_or(5);
        let _tpb = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads as usize)
            .build()?;
        let favs = match args.playlist.as_ref() {
            None => server.get_favs()?,
            Some(playlist_id) => server.get_playlist(playlist_id)?.playlist.songs,
        };
        let longest_title = favs
            .iter()
            .reduce(|acc, f| {
                if acc.title.len() < f.title.len() {
                    return f;
                };
                acc
            })
            .unwrap()
            .title
            .clone();

        let songs_done_counter = atomic::AtomicU32::new(0);
        favs.par_iter()
            .map(
                |fav| -> Result<(&libs::responses::SubSonicSong, Vec<Action>)> {
                    (|| {
                        let mut actions = vec![];

                        let song_path = format!(
                            "{}/.mp3/{}_{}",
                            rocksonic_dir,
                            fav.id,
                            args.mp3.unwrap_or(320)
                        );
                        let song_exists = fs::exists(&song_path)?;
                        if !song_exists {
                            actions.push(Action::Downloaded);
                            let mut res = server.get_song(&fav.id, args.mp3)?;
                            libs::utils::download_file(&mut res, &song_path)?;
                        }

                        let cover_path =
                            format!("{}/.cover/{}_{}", rocksonic_dir, fav.id, args.coversize);
                        let cover_exists = fs::exists(&cover_path)?;
                        if !cover_exists {
                            let mut cover_response =
                                server.get_cover_art(&fav.id, args.coversize)?;
                            download_file(&mut cover_response, &cover_path)?;
                            actions.push(Action::CoverDownloaded);
                        };

                        let converted_cover_path = format!(
                            "{}/.cover/{}_{}_baseline",
                            rocksonic_dir, fav.id, args.coversize
                        );
                        let converted_cover_exists = fs::exists(&converted_cover_path)?;
                        if !converted_cover_exists {
                            magick::convert_image(&cover_path, &converted_cover_path)?;

                            actions.push(Action::CoverConverted);
                        }
                        let suffix = if args.mp3.is_some() || fav.suffix == "opus" {
                            String::from("mp3")
                        } else {
                            fav.suffix.clone()
                        };

                        let combined_path = if args.flat {
                            let sanitized_song = sanitize_filename::sanitize(format!(
                                "{} {} {:0>3} {}.{}",
                                fav.artist,
                                fav.album,
                                fav.track.unwrap_or(0),
                                fav.title,
                                suffix
                            ));
                            format!("{}/{}/{}", output_dir, library_dir, sanitized_song)
                        } else {
                            let sanitized_artist = sanitize_filename::sanitize(&fav.artist);
                            let sanitized_album = sanitize_filename::sanitize(&fav.album);

                            let sanitized_directory = format!(
                                "{}/{}/{}/{}",
                                output_dir, library_dir, sanitized_artist, sanitized_album
                            );

                            if !fs::exists(&sanitized_directory)? {
                                fs::create_dir_all(&sanitized_directory)?;

                                let sanitized_cover_art =
                                    format!("{}/cover.jpeg", sanitized_directory);
                                fs::copy(&converted_cover_path, &sanitized_cover_art)?;
                            }
                            let sanitized_song = sanitize_filename::sanitize(format!(
                                "{:0>3} {}.{}",
                                fav.track.unwrap_or(0),
                                fav.title,
                                suffix
                            ));
                            format!("{}/{}", sanitized_directory, sanitized_song)
                        };
                        let combined_exists = fs::exists(&combined_path)?;
                        if !combined_exists {
                            ffmpeg::combine_song_and_cover(
                                &song_path,
                                &converted_cover_path,
                                &combined_path,
                            )?;
                            actions.push(Action::CoverEmbedded);

                            if args.mp3.is_some() {
                                actions.push(Action::Converted);
                            }
                        };

                        Ok((fav, actions))
                    })()
                    .map_err(|e: Error| anyhow!("{} {} {}", fav.title, fav.id, e))
                },
            )
            .for_each(|result| {
                print_status(result, &songs_done_counter, favs.len(), longest_title.len())
            });
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
    songs_done_counter: &AtomicU32,
    song_count: usize,
    title_spacing: usize,
) {
    let songs_done_count = songs_done_counter.fetch_add(1, atomic::Ordering::Release);
    match result {
        Ok((song, actions)) => {
            let mut actions_string = actions
                .iter()
                .map(|action| format!("{:?}", action))
                .collect::<Vec<String>>()
                .join(", ");
            if actions_string.is_empty() {
                actions_string = String::from("nothing to do");
            }

            println!(
                "{:-6}/{} {: ^width$} {}",
                songs_done_count + 1,
                song_count,
                song.title,
                actions_string,
                width = title_spacing
            )
        }
        Err(e) => {
            println!("{:-6}/{} {:?}", songs_done_count + 1, song_count, e)
        }
    }
}
