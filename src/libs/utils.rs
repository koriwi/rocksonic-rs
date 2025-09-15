use std::{fs::File, io};

use anyhow::Result;
use reqwest::blocking::Response;
const SANITIZE_OPTIONS: sanitize_filename::Options = sanitize_filename::Options {
    truncate: true,
    windows: false,
    replacement: "",
};

pub fn download_file(req_res: &mut Response, file_path: &str) -> Result<String> {
    let sanitized_file_path = sanitize_filename::sanitize_with_options(file_path, SANITIZE_OPTIONS);
    let mut file = File::create(format!(
        "./rocksonic_songs/.mp3/{}",
        sanitized_file_path.clone()
    ))?;
    io::copy(req_res, &mut file)?;
    Ok(sanitized_file_path)
}
