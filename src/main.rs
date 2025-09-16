pub mod libs;

use std::{fs, io::Read, process::Stdio, sync::atomic};

use crate::libs::{ffmpeg, server::Server};
use anyhow::{anyhow, Result};
use clap::Parser;
use colored::Colorize;
use rayon::prelude::*;

#[derive(Debug)]
enum Action {
    Downloaded,
    CoverExtracted,
    ConverConverted,
    MP3Converted,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None, disable_help_flag = true)]
struct Args {
    #[arg(short, long)]
    host: String,

    #[arg(short, long)]
    username: String,

    #[arg(short, long)]
    password: String,

    #[arg(long, action = clap::ArgAction::Help)]
    help: Option<bool>,

    #[arg(short, long)]
    mp3: bool,
}
fn setup_dirs() -> Result<()> {
    fs::create_dir_all("./rocksonic_songs/.mp3")?;
    fs::create_dir_all("./rocksonic_songs/.cover")?;
    Ok(())
}
fn main() -> Result<()> {
    setup_dirs()?;

    let args = Args::parse();
    let server = Server::connect(args.host, args.username, args.password).inspect_err(|_e| {
        println!("Could not connect to the server. Did you forget /rest ?");
    })?;

    println!("Welcome to {}!", "RockSonic".yellow().bold());
    println!("{}", "Successfully connected to SubSonic".green().italic());

    rayon::ThreadPoolBuilder::new()
        .num_threads(5)
        .build_global()?;

    let favs = server.get_favs()?;
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
    let done = atomic::AtomicI32::new(0);
    favs.par_iter()
        .map(
            |fav| -> Result<(&libs::responses::SubSonicSong, Vec<Action>)> {
                let mut actions = vec![];
                let song_path = format!("./rocksonic_songs/.mp3/{}", fav.id);
                let song_exists = fs::exists(&song_path).expect("could not check if song exists");
                if !song_exists {
                    actions.push(Action::Downloaded);
                    let mut res = server.get_song(&fav.id).unwrap();
                    libs::utils::download_file(&mut res, &fav.id).expect("could not download song");
                }
                let cover_path = format!("./rocksonic_songs/.cover/{}_orig", fav.id);
                let cover_exists =
                    fs::exists(&cover_path).expect("could not check if cover exists");
                if !cover_exists && ffmpeg::get_cover_stream(&song_path).is_some() {
                    ffmpeg::extract_cover(&song_path, &cover_path)?;
                    actions.push(Action::CoverExtracted);
                };
                Ok((fav, actions))
            },
        )
        .for_each(|result| {
            let done_count = done.fetch_add(1, atomic::Ordering::Release);
            match result {
                Ok((song, actions)) => {
                    println!(
                        "{:-6}/{} {: ^width$} {:?}",
                        done_count + 1,
                        favs.len(),
                        song.title,
                        actions,
                        width = longest_title.len()
                    )
                }
                Err(e) => {
                    println!("{:-6}/{} {:?}", done_count + 1, favs.len(), e)
                }
            }
        });

    Ok(())
}
