use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::{Context, Result, bail};
use ithmb_core::error::DecodedImage;
use ithmb_core::pipeline;
use ithmb_core::profile_db::ProfileDb;

use crate::cli_def::OutputFormat;

/// Decode a specific frame from a multi-frame file.
pub fn decode_frame(data: &[u8], frame: usize, db: &ProfileDb) -> Result<DecodedImage> {
    if data.len() < 4 {
        bail!("buffer too short: expected at least 4 bytes");
    }

    let prefix = i32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let is_jpeg = data[0] == 0xFF && data[1] == 0xD8;

    if is_jpeg {
        bail!("frame index {frame} out of range (JPEG files are single-frame)");
    }

    let profile = db
        .get(prefix)
        .ok_or_else(|| anyhow::anyhow!("unknown format prefix {prefix}"))?
        .clone();

    #[allow(clippy::cast_sign_loss)]
    let frame_size = profile.frame_size() as usize;
    let offset = 4 + frame * frame_size;
    let end = offset + frame_size;

    if end > data.len() {
        let max_frames = (data.len() - 4) / frame_size;
        bail!("frame index {frame} out of range: file has at most {max_frames} frame(s)");
    }

    let mut frame_buf = Vec::with_capacity(4 + frame_size);
    frame_buf.extend_from_slice(&data[..4]);
    frame_buf.extend_from_slice(&data[offset..end]);

    pipeline::decode_with_profile(&frame_buf, &profile, &std::sync::atomic::AtomicBool::new(false)).map_err(Into::into)
}

/// Open a PhotoDB/ArtworkDB container and extract all entries as numbered PNG files.
#[cfg(feature = "png-output")]
pub fn open_container(input: &Path) -> Result<()> {
    let data = fs::read(input).with_context(|| format!("failed to read '{}'", input.display()))?;
    let images = pipeline::open_ithmb(&data, &std::sync::atomic::AtomicBool::new(false), None)?;

    if images.is_empty() {
        bail!("no images found in container");
    }

    let out_dir = if let Some(parent) = input.parent() {
        let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
        parent.join(stem)
    } else {
        PathBuf::from("output")
    };
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create output directory '{}'", out_dir.display()))?;

    for (i, img) in images.iter().enumerate() {
        let n = i + 1;
        let mut path = out_dir.join(format!("thumb_{n:04}"));
        path.set_extension("png");
        write_png(img, &path)?;
        println!("Wrote {} ({}x{})", path.display(), img.width, img.height);
    }
    let len = images.len();
    eprintln!("Extracted {len} images to {}", out_dir.display());
    Ok(())
}

#[cfg(not(feature = "png-output"))]
pub fn open_container(_input: &Path) -> Result<()> {
    bail!("--open requires PNG encoding (rebuild with default features: `cargo build --features png-output`)");
}

/// Determine the output file path based on CLI settings.
pub fn resolve_output_path(input: &Path, output: Option<&PathBuf>, format: OutputFormat, raw: bool) -> PathBuf {
    if let Some(output) = output {
        return output.clone();
    }

    let use_png = should_use_png(None, format, raw);
    let mut path = input.to_path_buf();
    path.set_extension(if use_png { "png" } else { "bin" });
    path
}

/// Decide whether PNG encoding should be used for the output.
pub fn should_use_png(output: Option<&Path>, format: OutputFormat, raw: bool) -> bool {
    if raw {
        return false;
    }
    #[cfg(not(feature = "png-output"))]
    {
        let _ = format;
        let _ = output;
        return false;
    }
    #[cfg(feature = "png-output")]
    match format {
        OutputFormat::Png => true,
        OutputFormat::Bin => false,
        OutputFormat::Auto => output
            .and_then(|p| p.extension())
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png")),
    }
}

/// Write decoded pixel data as raw binary BGRA.
pub fn write_raw(img: &DecodedImage, path: &Path) -> io::Result<()> {
    fs::write(path, &img.data)
}

/// Write decoded pixel data as a PNG image (requires `png-output` feature).
#[cfg(feature = "png-output")]
pub fn write_png(img: &DecodedImage, path: &Path) -> Result<()> {
    use std::io::BufWriter;

    let file = fs::File::create(path).with_context(|| format!("failed to create '{}'", path.display()))?;
    let w = BufWriter::new(file);
    let mut encoder = png::Encoder::new(w, img.width, img.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().context("failed to write PNG header")?;
    writer
        .write_image_data(&img.data)
        .context("failed to write PNG image data")?;
    Ok(())
}
