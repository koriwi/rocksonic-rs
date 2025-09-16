use anyhow::{anyhow, Result};
use ffprobe::Stream;

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

pub fn extract_cover(song_path: &str, cover_path: &str) -> Result<()> {
    let mut command = ffmpeg_sidecar::command::FfmpegCommand::new()
        .input(song_path)
        .args(["-an"])
        .args(["-vcodec", "copy"])
        .args(["-f", "mjpeg"])
        .args(["-pix_fmt", "yuvj420p"])
        .args(["-color_range", "full"])
        .args(["-colorspace", "bt470bg"])
        .overwrite()
        .output(cover_path)
        .spawn()
        .expect("could not spawn ffmpeg");
    let status = command.wait()?; //.expect("could not await ffmpeg");
    if let Some(code) = status.code() {
        if code != 0 {
            return Err(anyhow!("ffmpeg exited ({})", code));
        };
    };
    Ok(())
}

pub fn get_cover_stream(path: &str) -> Option<Stream> {
    let info = ffprobe::ffprobe(path).ok()?;
    info.streams
        .into_iter()
        .find(|stream| stream.codec_type == Some(String::from("video")))
}
