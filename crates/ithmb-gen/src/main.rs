// SPDX-License-Identifier: MIT
//! ithmb-gen — sample .ithmb file generator for all 7 pixel encoding formats.

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

// ---------------------------------------------------------------------------
// CLI argument definitions
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Format {
    Rgb565,
    Rgb555,
    ReorderedRgb555,
    Uyvy,
    Ycbcr420,
    Clcl,
    Cl,
}

#[derive(Parser, Debug)]
#[command(name = "ithmb-gen", about = "Generate sample .ithmb thumbnail cache files")]
struct Args {
    /// Image width in pixels (default depends on format; use --recommended to see)
    #[arg(long)]
    width: Option<u32>,

    /// Image height in pixels (default depends on format; use --recommended to see)
    #[arg(long)]
    height: Option<u32>,

    /// Pixel encoding format (not required with --recommended)
    #[arg(long, required_unless_present = "recommended")]
    format: Option<Format>,

    /// Output file path
    #[arg(long, short, default_value = "output.ithmb")]
    output: PathBuf,

    /// Seed for deterministic pseudo-random output (omit for vertical gradient)
    #[arg(long)]
    seed: Option<u64>,

    /// Print recommended profile-matching dimensions for each format and exit
    #[arg(long)]
    recommended: bool,
}

// ---------------------------------------------------------------------------
// Pixel data generation
// ---------------------------------------------------------------------------

/// Generate BGRA pixel data.
///
/// Without a seed: vertical gradient (B = y-gradient, G = x-gradient, R = contrast).
/// With a seed: deterministic pseudo-random via a simple LCG.
#[allow(clippy::many_single_char_names, clippy::cast_possible_truncation)]
fn generate_pixels(w: u32, h: u32, seed: Option<u64>) -> Vec<u8> {
    let n = (w as usize) * (h as usize);
    let mut pixels = Vec::with_capacity(n * 4);

    match seed {
        None => {
            // Vertical gradient pattern
            let h_max = (h.saturating_sub(1)).max(1);
            let w_max = (w.saturating_sub(1)).max(1);
            for y in 0..h {
                for x in 0..w {
                    let b = (y * 255 / h_max) as u8;
                    let g = (x * 255 / w_max) as u8;
                    // Red provides contrast against the blue gradient
                    let r = 255u8.saturating_sub(b);
                    pixels.push(b);
                    pixels.push(g);
                    pixels.push(r);
                    pixels.push(255);
                }
            }
        }
        Some(mut state) => {
            // MMIX LCG — deterministic, portable, no dependencies
            let mut next = || -> u8 {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                (state >> 32) as u8
            };
            for _ in 0..n {
                pixels.push(next()); // B
                pixels.push(next()); // G
                pixels.push(next()); // R
                pixels.push(255); // A
            }
        }
    }

    pixels
}

// ---------------------------------------------------------------------------
// Encoder dispatch
// ---------------------------------------------------------------------------

/// Encode BGRA pixels using the selected format.
#[allow(clippy::cast_possible_wrap)]
fn encode(format: Format, bgra: &[u8], w: u32, h: u32) -> Vec<u8> {
    let w_i32 = w as i32;
    let h_i32 = h as i32;
    match format {
        Format::Rgb565 => ithmb_core::enc::encode_rgb565(bgra, w_i32, h_i32, true),
        Format::Rgb555 => ithmb_core::enc::encode_rgb555(bgra, w_i32, h_i32, true, false),
        Format::ReorderedRgb555 => ithmb_core::enc::encode_reordered_rgb555(bgra, w_i32, h_i32, true),
        Format::Uyvy => ithmb_core::enc::encode_uyvy(bgra, w_i32, h_i32),
        Format::Ycbcr420 => ithmb_core::enc::encode_ycbcr420(bgra, w_i32, h_i32, false),
        Format::Clcl => ithmb_core::enc::encode_clcl(bgra, w_i32, h_i32),
        Format::Cl => ithmb_core::enc::encode_cl(bgra, w_i32, h_i32),
    }
}

// ---------------------------------------------------------------------------
// Format default dimensions & recommended profiles
// ---------------------------------------------------------------------------

/// Returns the recommended dimensions for a format to match a known profile.
fn format_default_dim(fmt: Format) -> (u32, u32) {
    match fmt {
        Format::Rgb565 => (320, 240),
        Format::ReorderedRgb555 | Format::Clcl | Format::Cl => (256, 256),
        Format::Uyvy | Format::Ycbcr420 => (720, 480),
        Format::Rgb555 => (320, 320),
    }
}

/// Returns the hardcoded 4-byte prefix for a format.
fn format_prefix(fmt: Format) -> i32 {
    match fmt {
        Format::Rgb565 => 1024,
        Format::Rgb555 => 3005,
        Format::ReorderedRgb555 => 3001,
        Format::Uyvy => 1019,
        Format::Ycbcr420 => 1067,
        Format::Clcl => 9001,
        Format::Cl => 9002,
    }
}

/// Returns the human-readable format name.
fn format_name(fmt: Format) -> &'static str {
    match fmt {
        Format::Rgb565 => "RGB565",
        Format::Rgb555 => "RGB555",
        Format::ReorderedRgb555 => "Reordered RGB555",
        Format::Uyvy => "UYVY",
        Format::Ycbcr420 => "YCbCr 4:2:0",
        Format::Clcl => "CLCL",
        Format::Cl => "CL",
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // --recommended: print profile-matched dimensions and exit
    if args.recommended {
        println!("Profile-matched dimensions for each format:");
        println!("----------------------------------------");
        for fmt in [
            Format::Rgb565,
            Format::Rgb555,
            Format::ReorderedRgb555,
            Format::Uyvy,
            Format::Ycbcr420,
            Format::Clcl,
            Format::Cl,
        ] {
            let (w, h) = format_default_dim(fmt);
            let prefix = format_prefix(fmt);
            println!("  {:20} {}×{} (prefix {})", format_name(fmt), w, h, prefix);
        }
        return Ok(());
    }

    // Safe: --format is required unless --recommended is set
    let fmt = args.format.expect("--format is required without --recommended");

    // Use recommended dimensions if not explicitly provided
    let (def_w, def_h) = format_default_dim(fmt);
    let w = args.width.unwrap_or(def_w);
    let h = args.height.unwrap_or(def_h);

    if w != def_w || h != def_h {
        println!("Note: custom dimensions {w}×{h} may not match known profiles");
    }

    let pixels = generate_pixels(w, h, args.seed);
    let encoded = encode(fmt, &pixels, w, h);

    let prefix = format_prefix(fmt);

    let mut output = Vec::with_capacity(4 + encoded.len());
    output.extend_from_slice(&prefix.to_be_bytes());
    output.extend_from_slice(&encoded);

    std::fs::write(&args.output, &output)?;

    let fmt_name = format_name(fmt);
    println!(
        "Generated {}×{} {} .ithmb -> {} ({} bytes)",
        w,
        h,
        fmt_name,
        args.output.display(),
        output.len(),
    );

    Ok(())
}
