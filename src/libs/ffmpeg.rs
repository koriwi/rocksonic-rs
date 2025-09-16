use anyhow::{anyhow, Result};
use ffprobe::Stream;
//"pix_fmt": "yuvj420p", "color_range": "full", "colorspace": "bt470bg"
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
