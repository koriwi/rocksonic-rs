pub mod libs;

use std::{fs, io::Read, process::Stdio};

use crate::libs::{ffmpeg, server::Server};
use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use ffmpeg_sidecar::ffprobe;
use rayon::prelude::*;

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
    favs.par_iter().for_each(|fav| {
        let mut status_text = format!("{:-width$}:", fav.title, width = longest_title.len(),);
        let song_path = format!("./rocksonic_songs/.mp3/{}", fav.id);
        let song_exists = fs::exists(&song_path).expect("could not check if song exists");
        if !song_exists {
            status_text = format!("{} downloaded", status_text);
            let mut res = server.get_song(&fav.id).unwrap();
            libs::utils::download_file(&mut res, &fav.id).expect("could not download song");
        } else {
            status_text = format!("{} already downloaded", status_text);
        };

        let cover_path = format!("./rocksonic_songs/.cover/{}_orig", fav.id);
        let cover_exists = fs::exists(&cover_path).expect("could not check if cover exists");
        if !cover_exists && ffmpeg::get_cover(&song_path).is_some() {
            let mut command = ffmpeg_sidecar::command::FfmpegCommand::new()
                .input(&song_path)
                .args(["-an"])
                .args(["-vcodec", "copy"])
                .args(["-f", "mjpeg"])
                .overwrite()
                .output(cover_path)
                .spawn()
                .expect("could not spawn ffmpeg");
            let status = command.wait().expect("could not await ffmpeg");
            if let Some(code) = status.code() {
                if code != 0 {
                    status_text = format!("{}, ffmpeg exited ({})", status_text, code);
                };
            };
        } else {
            status_text = format!("{}, cover already downloaded", status_text);
        };
        println!("{}", status_text);
    });

    Ok(())
}
