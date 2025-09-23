use anyhow::{anyhow, Result};

pub fn convert_to_mp3(song_path: &str, mp3_path: &str) -> Result<()> {
    let mut command = ffmpeg_sidecar::command::FfmpegCommand::new()
        .input(song_path)
        .args(["-map", "0:a"])
        .args(["-metadata:s:v:0", "comment=\"Cover (front)\""])
        .args(["-metadata:s:v:0", "title=\"Album cover\""])
        .args(["-disposition:v:0", "attached_pic"])
        .args(["-id3v2_version", "3"])
        .overwrite()
        .output(mp3_path)
        .spawn()?;

    let status = command.wait()?; //.expect("could not await ffmpeg");
    if let Some(code) = status.code() {
        if code != 0 {
            return Err(anyhow!("ffmpeg exited ({})", code));
        };
    };
    Ok(())
}

pub fn combine_song_and_cover(
    song_path: &str,
    cover_path: &str,
    combined_path: &str,
) -> Result<()> {
    let mut command = ffmpeg_sidecar::command::FfmpegCommand::new()
        .input(song_path)
        .input(cover_path)
        .args(["-map", "0:a"])
        .args(["-map", "1:0"])
        .args(["-c:v", "copy"])
        .args(["-c:a", "copy"])
        .args(["-metadata:s:v:0", "comment=\"Cover (front)\""])
        .args(["-metadata:s:v:0", "title=\"Album cover\""])
        .args(["-disposition:v:0", "attached_pic"])
        .args(["-id3v2_version", "3"])
        .overwrite()
        .output(combined_path)
        .spawn()?;

    let status = command.wait()?; //.expect("could not await ffmpeg");
    if let Some(code) = status.code() {
        if code != 0 {
            return Err(anyhow!("ffmpeg exited ({})", code));
        };
    };
    Ok(())
}
