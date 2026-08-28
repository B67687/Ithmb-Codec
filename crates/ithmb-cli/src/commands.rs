use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use ithmb_core::profile_db::ProfileDb;

use crate::layout::resolve_frame_layout;

/// Print the number of frames (images) in an `.ithmb` file.
pub fn print_frame_count(input: &Path) -> Result<()> {
    let data = fs::read(input).with_context(|| format!("failed to read '{}'", input.display()))?;
    let db = ProfileDb::load_builtin().context("failed to load built-in profile database")?;
    let layout = resolve_frame_layout(&data, input, &db)?;
    println!("{}", layout.count);
    Ok(())
}

/// Extract every frame of an `.ithmb` file to its own `.ithmb` file.
pub fn extract_all(input: &Path) -> Result<()> {
    let data = fs::read(input).with_context(|| format!("failed to read '{}'", input.display()))?;
    let db = ProfileDb::load_builtin().context("failed to load built-in profile database")?;
    let layout = resolve_frame_layout(&data, input, &db)?;

    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let out_dir = if let Some(parent) = input.parent() {
        parent.join(stem)
    } else {
        std::path::PathBuf::from("output")
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

/// Print the known profile database as a formatted table.
pub fn list_profiles() -> Result<()> {
    let db = ProfileDb::load_builtin().context("failed to load built-in profile database")?;

    println!(
        "{:<8} {:<6} {:<6} {:<20} {:<16}",
        "Prefix", "Width", "Height", "Encoding", "FrameByteLength"
    );
    println!("{:-<8} {:-<6} {:-<6} {:-<20} {:-<16}", "", "", "", "", "");

    let mut keys: Vec<&i32> = db.all().keys().collect();
    keys.sort();

    for &key in &keys {
        let p = &db.all()[key];
        println!(
            "{:<8} {:<6} {:<6} {:<20?} {}",
            p.prefix, p.width, p.height, p.encoding, p.frame_byte_length
        );
    }

    Ok(())
}
