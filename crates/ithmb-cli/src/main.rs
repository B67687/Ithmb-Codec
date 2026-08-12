//! CLI tool for decoding `.ithmb` thumbnail cache files.
//!
//! Supports raw binary BGRA output and optional PNG encoding (default feature).

use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::{Context, Result, bail};
use clap::Parser;

use ithmb_core::error::DecodedImage;
use ithmb_core::profile_db::ProfileDb;
use ithmb_core::{self, Profile, pipeline};

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

    /// Print the number of frames (images) in the file
    #[arg(long)]
    frame_count: bool,

    /// Extract all frames to separate .ithmb files
    #[arg(long)]
    extract_all: bool,
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

    // --frame-count: print the number of frames and exit
    if cli.frame_count {
        return print_frame_count(input);
    }

    // --extract-all: write every frame to its own .ithmb file
    if cli.extract_all {
        return extract_all(input);
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

/// Open a PhotoDB/ArtworkDB container and extract all entries as numbered PNG files.
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
// Frame layout resolution (shared by --frame-count and --extract-all)
// ---------------------------------------------------------------------------

/// Parse the format id from an F-prefix filename such as `F1061_1.ithmb`.
///
/// Prefix-less F-files carry no numeric prefix in their content; the format id
/// lives in the filename (`F` + format id + `_{index}` + `.ithmb`).
fn f_filename_format_id(path: &Path) -> Option<i32> {
    let name = path.file_name()?.to_str()?;
    let after_f = name.strip_prefix('F')?;
    let digits: String = after_f.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

/// Frame layout of an `.ithmb` file: how individual frames are arranged in the
/// buffer and how extracted frames are re-serialized.
#[derive(Debug)]
struct FrameLayout {
    /// Number of frames in the file.
    count: usize,
    /// Byte offset of the first frame payload within the file buffer.
    data_offset: usize,
    /// Size in bytes of one frame payload.
    frame_size: usize,
    /// 4-byte big-endian format-id prefix prepended to extracted frames; `None`
    /// for JPEG streams, where the whole file is already one standalone frame.
    prefix_bytes: Option<[u8; 4]>,
}

impl FrameLayout {
    /// Payload bytes of frame `index` within the file buffer.
    fn frame_bytes<'a>(&self, data: &'a [u8], index: usize) -> &'a [u8] {
        let start = self.data_offset + index * self.frame_size;
        &data[start..start + self.frame_size]
    }

    /// Serialized bytes for extracted frame `index` (format-id prefix + payload).
    fn extracted_bytes(&self, data: &[u8], index: usize) -> Vec<u8> {
        let payload = self.frame_bytes(data, index);
        match self.prefix_bytes {
            Some(prefix) => {
                let mut out = Vec::with_capacity(4 + payload.len());
                out.extend_from_slice(&prefix);
                out.extend_from_slice(payload);
                out
            }
            None => payload.to_vec(),
        }
    }
}

/// Resolve how frames are laid out in `data` for the given input path.
///
/// Three layouts exist:
/// - a JPEG stream (`FF D8` SOI): single whole-file frame;
/// - a prefixed raw file `[format id][frame_0][frame_1]...`;
/// - a prefix-less F-file named `F{format_id}_{n}.ithmb`: `[frame_0]...`.
fn resolve_frame_layout(data: &[u8], input: &Path, db: &ProfileDb) -> Result<FrameLayout> {
    if data.len() < 4 {
        bail!("file too short: expected at least 4 bytes");
    }

    let prefix = i32::from_be_bytes([data[0], data[1], data[2], data[3]]);

    // JPEG streams carry no numeric prefix and are always single-frame.
    if data[0] == 0xFF && data[1] == 0xD8 {
        return Ok(FrameLayout {
            count: 1,
            data_offset: 0,
            frame_size: data.len(),
            prefix_bytes: None,
        });
    }

    // Prefixed file: the first four bytes are a known format id.
    if let Some(profile) = db.get(prefix) {
        return raw_layout(profile, data, 4);
    }

    // Prefix-less F-file: the format id comes from the filename.
    if let Some(format_id) = f_filename_format_id(input) {
        if let Some(profile) = db.get(format_id) {
            return raw_layout(profile, data, 0);
        }
        bail!("unknown format prefix {format_id} (from F-filename)");
    }

    bail!(
        "cannot determine format of '{}': unknown prefix {prefix}",
        input.display()
    );
}

/// Frame layout for a raw (non-JPEG) file with `frame_size`-aligned frames.
fn raw_layout(profile: &Profile, data: &[u8], data_offset: usize) -> Result<FrameLayout> {
    #[allow(clippy::cast_sign_loss)]
    let declared_frame_size = profile.frame_size() as usize;
    if declared_frame_size == 0 {
        bail!("profile {} has no frame size", profile.prefix);
    }
    #[allow(clippy::cast_sign_loss)]
    let prefix_bytes = (profile.prefix as u32).to_be_bytes();

    let payload_len = data.len() - data_offset;
    if payload_len == 0 {
        bail!("file too small to hold a full frame (0 payload bytes)");
    }
    // A payload smaller than one declared frame is a single-frame file whose
    // frame is the whole payload — e.g. YCbCr420 files whose profile frame size
    // (2 bytes/pixel) exceeds the actual 1.5 bytes/pixel payload.
    let frame_size = payload_len.min(declared_frame_size);
    let count = payload_len / frame_size;

    Ok(FrameLayout {
        count,
        data_offset,
        frame_size,
        prefix_bytes: Some(prefix_bytes),
    })
}

// ---------------------------------------------------------------------------
// Frame count mode
// ---------------------------------------------------------------------------

/// Print the number of frames (images) in an `.ithmb` file.
fn print_frame_count(input: &Path) -> Result<()> {
    let data = fs::read(input).with_context(|| format!("failed to read '{}'", input.display()))?;
    let db = ProfileDb::load_builtin().context("failed to load built-in profile database")?;
    let layout = resolve_frame_layout(&data, input, &db)?;
    println!("{}", layout.count);
    Ok(())
}

// ---------------------------------------------------------------------------
// Extract-all mode
// ---------------------------------------------------------------------------

/// Extract every frame of an `.ithmb` file to its own `.ithmb` file.
///
/// Each output file is a standalone single-frame file: the 4-byte format-id
/// prefix followed by that frame's payload. Outputs are written to a directory
/// named after the input file, one file per frame.
fn extract_all(input: &Path) -> Result<()> {
    let data = fs::read(input).with_context(|| format!("failed to read '{}'", input.display()))?;
    let db = ProfileDb::load_builtin().context("failed to load built-in profile database")?;
    let layout = resolve_frame_layout(&data, input, &db)?;

    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let out_dir = if let Some(parent) = input.parent() {
        parent.join(stem)
    } else {
        PathBuf::from("output")
    };
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create output directory '{}'", out_dir.display()))?;

    for i in 0..layout.count {
        let mut path = out_dir.join(format!("{stem}_{:04}", i + 1));
        path.set_extension("ithmb");
        let bytes = layout.extracted_bytes(&data, i);
        fs::write(&path, bytes).with_context(|| format!("failed to write '{}'", path.display()))?;
        println!("Wrote {}", path.display());
    }
    let count = layout.count;
    eprintln!("Extracted {count} frame(s) to {}", out_dir.display());
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn builtin_db() -> ProfileDb {
        ProfileDb::load_builtin().expect("built-in profile database loads")
    }

    /// `frames` zero-filled frames of `frame_size` preceded by the 4-byte prefix.
    #[allow(clippy::cast_sign_loss)]
    fn prefixed_buffer(prefix: i32, frame_size: usize, frames: usize) -> Vec<u8> {
        let mut buf = (prefix as u32).to_be_bytes().to_vec();
        buf.resize(4 + frame_size * frames, 0);
        buf
    }

    /// `frames` zero-filled prefix-less F-file frames of `frame_size`.
    fn f_buffer(frame_size: usize, frames: usize) -> Vec<u8> {
        vec![0; frame_size * frames]
    }

    #[test]
    fn f_filename_format_id_parses_f_names() {
        assert_eq!(f_filename_format_id(Path::new("F1061_1.ithmb")), Some(1061));
        assert_eq!(f_filename_format_id(Path::new("F1019_1.ithmb")), Some(1019));
        assert_eq!(f_filename_format_id(Path::new("F1055_1.ithmb")), Some(1055));
    }

    #[test]
    fn f_filename_format_id_rejects_other_names() {
        assert_eq!(f_filename_format_id(Path::new("sample.ithmb")), None);
        assert_eq!(f_filename_format_id(Path::new("F.ithmb")), None);
        assert_eq!(f_filename_format_id(Path::new("Fabc_1.ithmb")), None);
    }

    #[test]
    fn prefixed_multi_frame_layout() {
        let data = prefixed_buffer(1061, 6160, 10);
        let layout = resolve_frame_layout(&data, Path::new("F1061_1.ithmb"), &builtin_db()).unwrap();
        assert_eq!(layout.count, 10);
        assert_eq!(layout.data_offset, 4);
        assert_eq!(layout.frame_size, 6160);
        assert_eq!(layout.prefix_bytes, Some(1061_u32.to_be_bytes()));
    }

    #[test]
    fn prefixless_f_file_multi_frame_layout() {
        let data = f_buffer(6160, 10);
        let layout = resolve_frame_layout(&data, Path::new("F1061_1.ithmb"), &builtin_db()).unwrap();
        assert_eq!(layout.count, 10);
        assert_eq!(layout.data_offset, 0);
        assert_eq!(layout.frame_size, 6160);
        assert_eq!(layout.prefix_bytes, Some(1061_u32.to_be_bytes()));
    }

    #[test]
    fn single_frame_layouts() {
        let db = builtin_db();
        let prefixed =
            resolve_frame_layout(&prefixed_buffer(1024, 153_600, 1), Path::new("sample.ithmb"), &db).unwrap();
        assert_eq!(prefixed.count, 1);

        let f_file = resolve_frame_layout(&f_buffer(6160, 1), Path::new("F1061_1.ithmb"), &db).unwrap();
        assert_eq!(f_file.count, 1);
    }

    #[test]
    fn jpeg_stream_is_single_frame() {
        let mut data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        data.extend_from_slice(&[0u8; 64]);
        let layout = resolve_frame_layout(&data, Path::new("T1007.ithmb"), &builtin_db()).unwrap();
        assert_eq!(layout.count, 1);
        assert_eq!(layout.data_offset, 0);
        assert_eq!(layout.frame_size, data.len());
        assert_eq!(layout.prefix_bytes, None);
    }

    #[test]
    fn extracted_bytes_prepend_prefix() {
        let data = f_buffer(6160, 2);
        let layout = resolve_frame_layout(&data, Path::new("F1061_1.ithmb"), &builtin_db()).unwrap();
        let first = layout.extracted_bytes(&data, 0);
        let mut expected = 1061_u32.to_be_bytes().to_vec();
        expected.resize(4 + 6160, 0);
        assert_eq!(first, expected);
        assert_eq!(first.len(), 4 + 6160);
    }

    #[test]
    fn jpeg_extracted_bytes_are_whole_file() {
        let mut data = vec![0xAA; 96];
        data[0] = 0xFF;
        data[1] = 0xD8;
        let layout = resolve_frame_layout(&data, Path::new("T1007.ithmb"), &builtin_db()).unwrap();
        assert_eq!(layout.extracted_bytes(&data, 0), data);
    }

    #[test]
    fn unknown_format_errors() {
        let data = vec![0x12, 0x34, 0x56, 0x78, 0x00, 0x01, 0x02, 0x03];
        let err = resolve_frame_layout(&data, Path::new("mystery.ithmb"), &builtin_db())
            .expect_err("unknown prefix must fail");
        assert!(err.to_string().contains("cannot determine format"));
    }

    #[test]
    fn file_too_small_errors() {
        let db = builtin_db();
        // Prefix present but no payload: cannot hold a full frame.
        let err = resolve_frame_layout(&[0u8, 0, 4, 0x25], Path::new("F1061_1.ithmb"), &db)
            .expect_err("empty payload must fail");
        assert!(err.to_string().contains("too small"));

        // Shorter than 4 bytes.
        let err =
            resolve_frame_layout(&[0u8, 0, 0], Path::new("x.ithmb"), &db).expect_err("buffer under 4 bytes must fail");
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn payload_smaller_than_frame_size_is_single_frame() {
        // Profile 1067 (YCbCr420) declares 691200 bytes/frame, but real 720×480
        // 4:2:0 payloads are 518400 bytes (1.5 bytes/pixel).
        let mut buf = vec![0u8; 518_404];
        buf[..4].copy_from_slice(&1067_u32.to_be_bytes());
        let layout = resolve_frame_layout(&buf, Path::new("ycbcr420.ithmb"), &builtin_db()).unwrap();
        assert_eq!(layout.count, 1);
        assert_eq!(layout.frame_size, 518_400);
        assert_eq!(layout.extracted_bytes(&buf, 0), buf);
    }
}
