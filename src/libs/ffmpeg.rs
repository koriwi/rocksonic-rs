use ffprobe::Stream;

pub fn get_cover(path: &str) -> Option<Stream> {
    let info = ffprobe::ffprobe(path).ok()?;
    let cover_stream = info
        .streams
        .into_iter()
        .find(|stream| stream.codec_type == Some(String::from("video")));
    return cover_stream;
}
