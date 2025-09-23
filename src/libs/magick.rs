use anyhow::Result;
pub fn convert_image(input_path: &str, output_path: &str) -> Result<()> {
    let mut wand = magick_rust::MagickWand::new();
    wand.read_image(input_path)?;

    wand.set_compression_quality(75)?;
    wand.strip_image()?;
    wand.set_interlace_scheme(magick_rust::InterlaceType::No)?;
    wand.set_image_format("JPEG")?;
    wand.write_image(output_path)?;
    Ok(())
}
