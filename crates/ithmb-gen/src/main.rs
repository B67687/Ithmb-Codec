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
    /// Image width in pixels
    #[arg(long, default_value_t = 256)]
    width: u32,

    /// Image height in pixels
    #[arg(long, default_value_t = 256)]
    height: u32,

    /// Pixel encoding format
    #[arg(long)]
    format: Format,

    /// Output file path
    #[arg(long, short, default_value = "output.ithmb")]
    output: PathBuf,

    /// Seed for deterministic pseudo-random output (omit for vertical gradient)
    #[arg(long)]
    seed: Option<u64>,
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
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    let args = Args::parse();

    let pixels = generate_pixels(args.width, args.height, args.seed);
    let encoded = encode(args.format, &pixels, args.width, args.height);

    std::fs::write(&args.output, &encoded).expect("failed to write output file");

    // Format name for display (e.g. "reordered-rgb555" from the enum)
    let format_name = match args.format {
        Format::Rgb565 => "RGB565",
        Format::Rgb555 => "RGB555",
        Format::ReorderedRgb555 => "Reordered RGB555",
        Format::Uyvy => "UYVY",
        Format::Ycbcr420 => "YCbCr 4:2:0",
        Format::Clcl => "CLCL",
        Format::Cl => "CL",
    };

    println!(
        "Generated {}x{} {} .ithmb -> {} ({} bytes)",
        args.width,
        args.height,
        format_name,
        args.output.display(),
        encoded.len(),
    );
}
