//! CLI tool for decoding `.ithmb` thumbnail cache files.
//!
//! Supports raw binary BGRA output and optional PNG encoding (default feature).

use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::{Context, Result, bail};
use clap::Parser;

use ithmb_core::error::DecodedImage;
use ithmb_core::profile_db::ProfileDb;
use ithmb_core::{self, pipeline};

// ---------------------------------------------------------------------------
// CLI argument types
// ---------------------------------------------------------------------------

/// Output format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum OutputFormat {
    /// Auto-detect from output file extension
    Auto,
    /// Raw binary BGRA data
    Bin,
    /// PNG image
    Png,
}

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

/// .ithmb image decoder
#[derive(Parser)]
#[command(name = "ithmb", version, about)]
#[allow(clippy::struct_excessive_bools)]
struct Cli {
    /// Input .ithmb file path
    input: Option<PathBuf>,

    /// Output file path (optional: defaults to input name with .png/.bin)
    output: Option<PathBuf>,

    /// Output format (default: auto-detect from extension)
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Auto)]
    format: OutputFormat,

    /// Frame index for multi-frame files
    #[arg(long, default_value_t = 0)]
    frame: usize,

    /// List all known profiles and exit
    #[arg(long)]
    list_profiles: bool,

    /// Dump raw BGRA output (no PNG conversion)
    #[arg(short, long)]
    raw: bool,

    /// Print metadata only, don't decode pixels
    #[arg(long)]
    info: bool,

    /// Open a PhotoDB/ArtworkDB container and extract all entries
    #[arg(long)]
    open: bool,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let cli = Cli::parse();

    // --list-profiles: print table and exit
    if cli.list_profiles {
        return list_profiles();
    }

    // Input file is required for all other modes
    let input = cli
        .input
        .as_deref()
        .context("input file is required (use --help for usage)")?;

    // --info: print metadata and exit
    if cli.info {
        return print_info(input);
    }

    // --open: process PhotoDB/ArtworkDB container
    if cli.open {
        return open_container(input);
    }

    // -- Decode path --
    let data = fs::read(input).with_context(|| format!("failed to read '{}'", input.display()))?;

    let db = ProfileDb::load_builtin().context("failed to load built-in profile database")?;

    let img = if cli.frame == 0 {
        pipeline::decode_ithmb(&data, &std::sync::atomic::AtomicBool::new(false))?
    } else {
        decode_frame(&data, cli.frame, &db)?
    };

    let output = resolve_output_path(input, cli.output.as_ref(), cli.format, cli.raw);

    #[cfg(feature = "png-output")]
    if should_use_png(Some(&output), cli.format, cli.raw) {
        return write_png(&img, &output).with_context(|| format!("failed to write PNG to '{}'", output.display()));
    }

    write_raw(&img, &output).with_context(|| format!("failed to write to '{}'", output.display()))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Frame extraction
// ---------------------------------------------------------------------------

/// Decode a specific frame from a multi-frame file.
fn decode_frame(data: &[u8], frame: usize, db: &ProfileDb) -> Result<DecodedImage> {
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

// ---------------------------------------------------------------------------
// Container (PhotoDB / ArtworkDB) extraction
// ---------------------------------------------------------------------------

/// Open a PhotoDB/ArtworkDB container and extract all entries as numbered PNGs.
#[cfg(feature = "png-output")]
fn open_container(input: &Path) -> Result<()> {
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
fn open_container(_input: &Path) -> Result<()> {
    bail!("--open requires PNG encoding (rebuild with default features: `cargo build --features png-output`)");
}

// ---------------------------------------------------------------------------
// Profile table
// ---------------------------------------------------------------------------

/// Print the known profile database as a formatted table.
fn list_profiles() -> Result<()> {
    let db = ProfileDb::load_builtin().context("failed to load built-in profile database")?;

    println!(
        "{:<8} {:<6} {:<6} {:<20} {:<16}",
        "Prefix", "Width", "Height", "Encoding", "FrameByteLength"
    );
    println!("{:-<8} {:-<6} {:-<6} {:-<20} {:-<16}", "", "", "", "", "");

    let mut keys: Vec<&i32> = db.all().keys().collect();
    keys.sort();

    for &key in &keys {
        // SAFETY: key came from the map directly
        let p = &db.all()[key];
        println!(
            "{:<8} {:<6} {:<6} {:<20?} {}",
            p.prefix, p.width, p.height, p.encoding, p.frame_byte_length
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Info mode
// ---------------------------------------------------------------------------

/// Read and print file metadata without decoding pixel data.
fn print_info(input: &Path) -> Result<()> {
    let metadata = fs::metadata(input).with_context(|| format!("failed to read metadata for '{}'", input.display()))?;
    let file_size = metadata.len();

    let data = fs::read(input).with_context(|| format!("failed to read '{}'", input.display()))?;

    println!("File:  {}", input.display());
    println!("Size:  {file_size} bytes");

    if data.len() < 4 {
        println!("Prefix: (file too short)");
        return Ok(());
    }

    let prefix = i32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let is_jpeg = data[0] == 0xFF && data[1] == 0xD8;

    if is_jpeg {
        println!("Prefix: JPEG stream (embedded JPEG)");
        println!("Profile: JPEG (no profile lookup needed)");
        return Ok(());
    }

    println!("Prefix: {prefix}");

    let db = ProfileDb::load_builtin().context("failed to load built-in profile database")?;

    match db.get(prefix) {
        Some(profile) => {
            println!(
                "Profile: {} ({}×{}, {:?}, {} bytes/frame)",
                profile.prefix, profile.width, profile.height, profile.encoding, profile.frame_byte_length
            );

            #[allow(clippy::cast_sign_loss)]
            let frame_size = profile.frame_size() as usize;
            let pixel_data_len = data.len().saturating_sub(4);
            let num_frames = pixel_data_len.checked_div(frame_size).unwrap_or(1);
            println!("Frames:  {}", num_frames.max(1));

            if profile.swaps_dimensions {
                println!(
                    "Display: {}×{} (swapped)",
                    profile.display_width(),
                    profile.display_height()
                );
            }
            if profile.is_interlaced {
                println!("Interlaced: yes");
            }
            if profile.is_padded {
                println!("Padded: yes (slot size: {} bytes)", profile.slot_size);
            }
            if profile.rotation != 0 {
                println!("Rotation: {}°", profile.rotation);
            }
            if profile.crop_x != 0 || profile.crop_y != 0 || profile.crop_width != 0 || profile.crop_height != 0 {
                println!(
                    "Crop: x={}, y={}, w={}, h={}",
                    profile.crop_x, profile.crop_y, profile.crop_width, profile.crop_height
                );
            }
        }
        None => {
            println!("Profile: unknown (not found in built-in database)");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

/// Determine the output file path based on CLI settings.
fn resolve_output_path(input: &Path, output: Option<&PathBuf>, format: OutputFormat, raw: bool) -> PathBuf {
    if let Some(output) = output {
        return output.clone();
    }

    let use_png = should_use_png(None, format, raw);
    let mut path = input.to_path_buf();
    path.set_extension(if use_png { "png" } else { "bin" });
    path
}

/// Decide whether PNG encoding should be used for the output.
fn should_use_png(output: Option<&Path>, format: OutputFormat, raw: bool) -> bool {
    if raw {
        return false;
    }
    #[cfg(not(feature = "png-output"))]
    {
        // Without the png-output feature, PNG encoding is unavailable.
        // If Png was explicitly requested, the user gets a .bin fallback.
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

// ---------------------------------------------------------------------------
// Output writers
// ---------------------------------------------------------------------------

/// Write decoded pixel data as raw binary BGRA.
fn write_raw(img: &DecodedImage, path: &Path) -> io::Result<()> {
    fs::write(path, &img.data)
}

/// Write decoded pixel data as a PNG image (requires `png-output` feature).
#[cfg(feature = "png-output")]
fn write_png(img: &DecodedImage, path: &Path) -> Result<()> {
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
