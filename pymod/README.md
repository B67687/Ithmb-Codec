# ithmb-core Python Bindings

Python bindings for [Ithmb-Codec](https://github.com/B67687/Ithmb-Codec), a pure Rust decoder for Apple `.ithmb` thumbnail cache files.

## Build

```bash
# Install maturin if you don't have it
pip install maturin

# Build and install the package
cd pymod/
maturin develop --release
```

## Usage

```python
import ithmb_core

# Decode a single .ithmb file
with open("photo.ithmb", "rb") as f:
    data = f.read()
result = ithmb_core.decode_ithmb(data)
# result = { "width": 320, "height": 240, "data": <BGRA bytes>, "format": "BGRA", "rotation": 0 }

# Decode a PhotoDB/ArtworkDB container
with open("PhotoDB", "rb") as f:
    data = f.read()
images = ithmb_core.open_ithmb(data)
for img in images:
    print(f"{img['width']}x{img['height']}")

# List all known profiles
profiles = ithmb_core.list_profiles()
for p in profiles:
    print(f"{p['name']}: {p['width']}x{p['height']} ({p['encoding']})")
```

## API

- `decode_ithmb(data, canceled=None)` — Decode a single `.ithmb` file from bytes. Returns a dict with `width`, `height`, `data` (BGRA `bytes`), `format`, `rotation`.
- `open_ithmb(data, canceled=None)` — Decode a PhotoDB/ArtworkDB container or bare `.ithmb`. Returns a list of dicts (same shape as `decode_ithmb`).
- `list_profiles()` — List all 54 known decoding profiles.
