# ithmb-cli

CLI tool for decoding Apple `.ithmb` thumbnail cache files from iPods and iPhones.

Part of the [Ithmb-Codec](https://github.com/B67687/Ithmb-Codec) workspace — the authoritative Rust codec for `.ithmb` files.

## Install

```bash
cargo install ithmb-cli
```

## Usage

```bash
# Decode a single .ithmb file to PNG
ithmb my_photo.ithmb output.png

# Decode to raw BGRA
ithmb my_photo.ithmb output.bin

# Open a PhotoDB container and extract all thumbnails
ithmb --open PhotoDB

# Select a specific frame from a multi-frame file
ithmb my_photo.ithmb --index 2

# List all 54 known decoding profiles
ithmb --list-profiles

# Forward raw data to stdout (pipe to another tool)
ithmb my_photo.ithmb --format bin -o - | ffmpeg -f rawvideo -pix_fmt bgra -s 320x240 -i - output.png
```

## Features

- Decodes all 8 raw pixel formats (RGB565, RGB555, ReorderedRGB555, UYVY, YCbCr420, CLCL, CL, JPEG)
- PhotoDB/ArtworkDB container parsing
- Auto-detection of embedded JPEG data (T-prefix files)
- PNG output (default feature)
- Frame index selection for multi-frame files
- Profile listing

## Build from source

```bash
git clone https://github.com/B67687/Ithmb-Codec.git
cd Ithmb-Codec
cargo build -p ithmb-cli --release
./target/release/ithmb --help
```

## License

MIT — see [LICENSE](https://github.com/B67687/Ithmb-Codec/blob/main/LICENSE) in the repository root.
