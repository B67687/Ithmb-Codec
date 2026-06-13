# Hardware Validation Guide

Buy an old iPod → extract `.ithmb` files → validate every decoder in our codec against real hardware output.

---

## Which iPod to Buy

### Primary target: iPod Classic 5.5G ("iPod with Video") 80GB Enhanced

- **Model:** A1136 (80GB) or A1238 (160GB)
- **Why:** 720×480 TV-out photos produce F1019 (YUV422 interlaced) — our most complex untested decoder
- **Price:** ~$20-40 USD / ~PHP 1,000-2,000 / ~RM 60-150
- **Also validates:** F1015, F1024, F1028, F1029, F1036, F1066, F1016, F1017, F1031 (RGB565 profiles)

### Secondary target: iPod Nano 3G

- **Model:** A1236
- **Why:** Uses F1067 (YCbCr 4:2:0 padded) — the only profile using this decoder engine
- **Price:** ~$15-25 USD
- **Also validates:** F1024, F1066, F1060, F1055 (RGB565 cover art)

### What to look for in listings

- "Tested working" — the hard drive and battery are the failure points
- Photos of the iPod's screen turned on
- Original USB cable included (30-pin)
- Charging and syncing requires a **30-pin to USB cable**

---

## Step 1: Sync Photos to the iPod

### On Windows:

1. Install iTunes 12.x (last version supporting old iPods) or **iTunes 10.7** (more reliable)
2. Connect iPod via 30-pin USB cable
3. iTunes should detect it — may ask to "Restore" if the disk is corrupted
4. Select photos to sync (any folder with JPEGs will do)
5. Sync — iTunes creates `Photos/Thumbs/F####_N.ithmb` files on the iPod

### On macOS (older versions):

1. Use **iTunes** or **Image Capture** to sync photos
2. Same process as Windows

### Alternative — use existing photos already on the iPod:

If the previous owner left photos on it, they're already cached as `.ithmb` files.
Skip to Step 2.

---

## Step 2: Extract the Photo Cache

### On Windows (using Explorer):

1. Enable **"Enable disk use"** in iTunes iPod settings
2. The iPod appears as a removable drive in My Computer
3. Navigate to `iPod_Control\Photos\Thumbs\`
4. You'll see files like:
   - `F1019_1.ithmb` — YUV422 interlaced (720×480)
   - `F1015_1.ithmb` — RGB565 (130×88)
   - `F1024_1.ithmb` — RGB565 (320×240)
   - `F1066_1.ithmb` — RGB565 (64×64)
5. Copy all `.ithmb` files to a folder on your PC
6. Also copy `Photos\Photo Database` (the index file)

### On macOS / Linux:

Mac: The iPod appears as a mounted volume. Navigate similarly.
Linux: Use `ifuse` to mount the iPod filesystem (requires libimobiledevice).

---

## Step 3: Decode with Our Codec

Run the CLI decoder tool on each `.ithmb` file:

```bash
# From the repo root:
dotnet run --project tools/IthmbDecoder -c Release -- F1019_1.ithmb F1019_decoded.bmp
dotnet run --project tools/IthmbDecoder -c Release -- F1024_1.ithmb F1024_decoded.bmp
dotnet run --project tools/IthmbDecoder -c Release -- F1066_1.ithmb F1066_decoded.bmp
# ...etc for each file
```

### Expected results:

| File            | Expected                   | If it fails                             |
| --------------- | -------------------------- | --------------------------------------- |
| `F1019_1.ithmb` | 720×480 BMP, correct photo | YUV422 interlaced decoder needs fixing  |
| `F1024_1.ithmb` | 320×240 BMP                | RGB565 decoder issue                    |
| `F1066_1.ithmb` | 64×64 BMP                  | RGB565 decoder issue                    |
| `F1067_1.ithmb` | 720×480 BMP (Nano 3G only) | YCbCr 4:2:0 padded decoder needs fixing |

---

## Step 4: Cross-Validate Against Known-Good Decoders

### Option A: iOpenPod (Python, macOS/Linux/Windows)

```bash
git clone https://github.com/TheRealSavi/iOpenPod.git
cd iOpenPod
pip install -r requirements.txt
python -m ipod_device.dump --decode F1019_1.ithmb --output decoded.png
```

### Option B: iThmb Converter (Windows, commercial)

Download trial from https://www.ithmbconverter.com — it extracts JPEGs from T-prefix files.
May not support all F-prefix formats.

### Compare:

Compare the BMP from our decoder with the PNG from iOpenPod.
They should be visually identical.
For RGB565: byte-perfect match expected.
For YUV: ±1-3 per channel due to BT.601 rounding differences.

---

## Step 5: If Our Decoder Fails — Debug

1. **Check the file prefix** — first 4 bytes as big-endian int32:

   ```bash
   xxd -l 4 F1019_1.ithmb
   ```

   Should match a profile in our table (e.g., 1019 = `0x000003FB`)

2. **Check file size** — Does it match `frameBytes` in our profile table?

   ```bash
   stat -c %s F1019_1.ithmb
   ```

3. **Check for embedded JPEG** (T-prefix inside F-named file):

   ```bash
   xxd F1019_1.ithmb | grep "ffd8"
   ```

4. **Hex dump the first 128 bytes** — compare against the format spec in the README:

   ```bash
   xxd -l 128 F1019_1.ithmb
   ```

5. **Open an issue** on GitHub with the hex dump, file size, and iPod model.
   Or fix the decoder yourself and submit a PR.

---

## Quick Reference: Which Format Each iPod Produces

| iPod Model             | Likely formats                     | Decoder tested          |
| ---------------------- | ---------------------------------- | ----------------------- |
| **iPod Photo 4G**      | 1009, 1013, 1015, 1019, 1016, 1017 | RGB565 ✅               |
| **iPod Video 5G/5.5G** | 1036, 1024, 1015, 1019, 1028, 1029 | YUV422 ⚠️ untested      |
| **iPod Classic 5G/6G** | 1067, 1024, 1066, 1055, 1060       | YCbCr 4:2:0 ⚠️ untested |
| **iPod Nano 3G**       | 1067, 1024, 1066                   | YCbCr 4:2:0 ⚠️ untested |
| **iPod Nano 4G**       | 1024, 1066, 1079, 1083             | RGB565 ✅               |
| **iPod Nano 6G**       | 1092, 1093                         | RGB565 ✅               |

✅ = Unit-tested with synthetic data (roundtrip proven)
⚠️ = Codec exists but never validated against real hardware

---

## Share the Results

Once you've validated (or fixed) a decoder, consider:

1. Opening a PR with your test files and findings
2. Publishing the F-prefix sample files as a public corpus (they're your own photos — your choice)
3. Publishing sample files (your own photos — your choice)

---

## Summary Checklist

- [ ] Buy iPod Classic 5.5G (or Nano 3G) with working HDD + battery
- [ ] Sync photos via iTunes (or find existing photos on device)
- [ ] Enable disk mode, extract `iPod_Control/Photos/Thumbs/`
- [ ] Run `IthmbDecoder` on each `.ithmb` file
- [ ] Compare output with iOpenPod decode
- [ ] If mismatched: debug and fix the decoder
- [ ] If matched: mark the decoder as validated against real hardware
