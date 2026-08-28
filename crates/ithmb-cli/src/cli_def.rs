use std::path::PathBuf;

use clap::Parser;

/// Output format selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Auto-detect from output file extension
    Auto,
    /// Raw binary BGRA data
    Bin,
    /// PNG image
    Png,
}

/// .ithmb image decoder
#[derive(Parser)]
#[command(name = "ithmb", version, about)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    /// Input .ithmb file path
    pub input: Option<PathBuf>,

    /// Output file path (optional: defaults to input name with .png/.bin)
    pub output: Option<PathBuf>,

    /// Output format (default: auto-detect from extension)
    #[arg(short, long, value_enum, default_value_t = OutputFormat::Auto)]
    pub format: OutputFormat,

    /// Frame index for multi-frame files
    #[arg(long, default_value_t = 0)]
    pub frame: usize,

    /// List all known profiles and exit
    #[arg(long)]
    pub list_profiles: bool,

    /// Dump raw BGRA output (no PNG conversion)
    #[arg(short, long)]
    pub raw: bool,

    /// Print metadata only, don't decode pixels
    #[arg(long)]
    pub info: bool,

    /// Open a PhotoDB/ArtworkDB container and extract all entries
    #[arg(long)]
    pub open: bool,

    /// Print the number of frames (images) in the file
    #[arg(long)]
    pub frame_count: bool,

    /// Extract all frames to separate .ithmb files
    #[arg(long)]
    pub extract_all: bool,
}
