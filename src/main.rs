pub mod libs;

use crate::libs::server::Server;
use anyhow::Result;
use clap::Parser;
use colored::Colorize;

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
fn main() -> Result<()> {
    let args = Args::parse();
    let server = Server::connect(args.host, args.username, args.password);
    if let Err(e) = server {
        println!("Could not connect to the server. Did you forget /rest ?");
        return Err(e);
    };
    println!("Welcome to {}!", "RockSonic".yellow().bold());
    println!("{}","Successfully connected to SubSonic".green().italic());
    Ok(())
}
