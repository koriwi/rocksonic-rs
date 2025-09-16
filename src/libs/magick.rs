use anyhow::Result;
pub fn convert_image(input_path: &str, output_path: &str, width_px: u16) -> Result<()> {
    let mut wand = magick_rust::MagickWand::new();
    wand.read_image(input_path)?;

    let aspect_ratio = wand.get_image_width() as f64 / wand.get_image_height() as f64;
    let scale = width_px as f64 / wand.get_image_width() as f64;
    wand.scale_image(
        scale,
        scale * aspect_ratio,
        magick_rust::FilterType::Lanczos,
    )?;

    wand.set_compression_quality(75)?;
    wand.strip_image()?;
    wand.set_interlace_scheme(magick_rust::InterlaceType::No)?;
    wand.write_image(output_path)?;
    Ok(())
}
