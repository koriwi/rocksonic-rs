pub mod libs;
use magick_rust::magick_wand_genesis;
use std::{
    fs,
    sync::atomic::{self, AtomicU32},
};

use crate::libs::{ffmpeg, magick, responses::SubSonicSong, server::Server};
use anyhow::{anyhow, Error, Result};
use clap::Parser;
use colored::Colorize;
use rayon::prelude::*;

#[derive(Debug)]
enum Action {
    Downloaded,
    CoverExtracted,
    CoverConverted,
    Converted,
    CoverEmbedded,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, disable_help_flag = true)]
struct Args {
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

    #[arg(
        short,
        long,
        help = "put all files in one folder. puts the artist and album name in the file name"
    )]
    flat: bool,

    #[arg(short = 'l', long, help = "use fav (liked songs) or the <playlist-id>")]
    playlist: Option<String>,
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

fn setup_dirs(library_dir: &str) -> Result<()> {
    fs::create_dir_all("./rocksonic_songs/.mp3")?;
    fs::create_dir_all("./rocksonic_songs/.cover")?;
    fs::create_dir_all(library_dir)?;
    Ok(())
}

fn main() -> Result<()> {
    magick_wand_genesis();
    let args = Args::parse();

    let server = Server::connect(args.host, args.username, args.password).inspect_err(|_e| {
        println!("Could not connect to the server. Did you forget /rest ?");
    })?;

    let mut library_dir = String::from("./rocksonic_songs/");
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
    setup_dirs(&library_dir)?;

    println!("Welcome to {}!", "RockSonic".yellow().bold());
    println!("{}", "Successfully connected to SubSonic".green().italic());

    let num_threads = args.threads.unwrap_or(5);
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads as usize)
        .build_global()?;

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
                        "./rocksonic_songs/.mp3/{}_{}",
                        fav.id,
                        args.mp3.unwrap_or(320)
                    );
                    let song_exists = fs::exists(&song_path)?;
                    if !song_exists {
                        actions.push(Action::Downloaded);
                        let mut res = server.get_song(&fav.id, args.mp3)?;
                        libs::utils::download_file(&mut res, &song_path)?;
                    }

                    let cover_path = format!("./rocksonic_songs/.cover/{}_orig", fav.id);
                    let cover_exists = fs::exists(&cover_path)?;
                    let song_has_cover =
                        cover_exists || ffmpeg::get_cover_stream(&song_path).is_some();
                    if !cover_exists && song_has_cover {
                        ffmpeg::extract_cover(&song_path, &cover_path)?;
                        actions.push(Action::CoverExtracted);
                    };

                    let converted_cover_path =
                        format!("./rocksonic_songs/.cover/{}_{}", fav.id, args.coversize);
                    let converted_cover_exists = fs::exists(&converted_cover_path)?;
                    if !converted_cover_exists && song_has_cover {
                        magick::convert_image(&cover_path, &converted_cover_path, args.coversize)?;

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
                        format!("{}/{}", library_dir, sanitized_song)
                    } else {
                        let sanitized_artist = sanitize_filename::sanitize(&fav.artist);
                        let sanitized_album = sanitize_filename::sanitize(&fav.album);

                        let sanitized_directory =
                            format!("{}/{}/{}", library_dir, sanitized_artist, sanitized_album);
                        fs::create_dir_all(&sanitized_directory)?;

                        let sanitized_song = sanitize_filename::sanitize(format!(
                            "{:0>3} {}.{}",
                            fav.track.unwrap_or(0),
                            fav.title,
                            suffix
                        ));
                        format!("{}/{}", sanitized_directory, sanitized_song)
                    };
                    // let combined_path = format!("{}/{}", library_dir, sanitized_filename);
                    let combined_exists = fs::exists(&combined_path)?;
                    if !combined_exists {
                        if song_has_cover {
                            ffmpeg::combine_song_and_cover(
                                &song_path,
                                &converted_cover_path,
                                &combined_path,
                            )?;
                            actions.push(Action::CoverEmbedded)
                        } else if args.mp3.is_none() {
                            fs::hard_link(song_path, combined_path)?;
                        } else {
                            ffmpeg::convert_to_mp3(&song_path, &combined_path)?;
                        };

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

    Ok(())
}
