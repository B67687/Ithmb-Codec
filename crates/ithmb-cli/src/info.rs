use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use ithmb_core::profile_db::ProfileDb;

/// Read and print file metadata without decoding pixel data.
pub fn print_info(input: &Path) -> Result<()> {
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
