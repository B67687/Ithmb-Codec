//! CLI tool for decoding `.ithmb` thumbnail cache files.
//!
//! Supports raw binary BGRA output and optional PNG encoding (default feature).

mod cli_def;
mod commands;
mod info;
mod io_helpers;
mod layout;

use anyhow::Result;
use clap::Parser;

use cli_def::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();

    // --list-profiles: print table and exit
    if cli.list_profiles {
        return commands::list_profiles();
    }

    // Input file is required for all other modes
    let input = cli
        .input
        .as_deref()
        .context("input file is required (use --help for usage)")?;

    // --info: print metadata and exit
    if cli.info {
        return info::print_info(input);
    }

    // --open: process PhotoDB/ArtworkDB container
    if cli.open {
        return io_helpers::open_container(input);
    }

    // --frame-count: print the number of frames and exit
    if cli.frame_count {
        return commands::print_frame_count(input);
    }

    // --extract-all: write every frame to its own .ithmb file
    if cli.extract_all {
        return commands::extract_all(input);
    }

    // -- Decode path --
    let data = std::fs::read(input).with_context(|| format!("failed to read '{}'", input.display()))?;

    let db = ithmb_core::profile_db::ProfileDb::load_builtin().context("failed to load built-in profile database")?;

    let img = if cli.frame == 0 {
        ithmb_core::pipeline::decode_ithmb(&data, &std::sync::atomic::AtomicBool::new(false))?
    } else {
        io_helpers::decode_frame(&data, cli.frame, &db)?
    };

    let output = io_helpers::resolve_output_path(input, cli.output.as_ref(), cli.format, cli.raw);

    #[cfg(feature = "png-output")]
    if io_helpers::should_use_png(Some(&output), cli.format, cli.raw) {
        return io_helpers::write_png(&img, &output)
            .with_context(|| format!("failed to write PNG to '{}'", output.display()));
    }

    io_helpers::write_raw(&img, &output).with_context(|| format!("failed to write to '{}'", output.display()))?;

    Ok(())
}

use anyhow::Context;
