# iOpenPod Reference Analysis

**Generated:** 2026-06-28

**Sources:** `/tmp/iOpenPod/` (cloned from https://github.com/TheRealSavi/iOpenPod, commit `b4ec6ba`)

**Our project:** `/home/nami/projects/dev/Ithmb-Codec/`

---

## Summary

| Area | Verdict |
|------|---------|
|| Format profile tables (ARTWORK_FORMATS_BY_ID vs KnownProfiles) | ⚠️ Partial (43 IDs match well; 6+ Nano-7G overrides missing; 1081 JPEG→RGB565 intentional) |
| Device capabilities (17 device profiles) | ⚠️ Partial — many discrepancies in per-device format ID assignments |
| _crop_visible_region vs our crop logic | ⚠️ Partial — same concept, different implementation |
| _resolve_packed_geometry trailing-trim threshold | ✅ Authoritative (256 bytes matches) |
| PackRgb565 reference | ✅ Authoritative (identical bit packing) |
| RgbToYuv BT.601 coefficients | ❌ Incorrect — test claims to match iOpenPod but uses full-range (JPEG) BT.601 vs iOpenPod's studio-range BT.601 |
| PhotoDB chunk structure | ✅ Authoritative (MHFD/MHSD/MHLI/MHII/MHNI structure matches) |
| Format 1013 rotation (big-endian, 90°) | ✅ Authoritative (220×176, BE, 90° rotation confirmed) |
| Format 1081 encoding | ⚠️ Partial — iOpenPod says JPEG, we use RGB565 (documented deviation) |
| Multi-frame concatenation | ⚠️ Partial — iOpenPod concatenates in photo pipeline, but our mechanism differs |
|| REC_RGB555_LE for 3001/3002/3003 | ✅ Correct (empirically verified by iOpenPod creator; iOpenPod's codec is incomplete — treats as plain RGB555_LE) |

---

## Detailed Analysis

### 1. Format Profile Tables: ARTWORK_FORMATS_BY_ID vs KnownProfiles

**Source (iOpenPod):** `/tmp/iOpenPod/ipod_device/artwork_presets.py` — `ARTWORK_FORMATS_BY_ID` dict
**Our file:** `IthmbCodecPlugin.ProfileSystem.cs` — `GetBuiltInProfiles()` method

#### Matching Profiles (✅ Authoritative)

The following format IDs have identical dimensions and compatible encoding between iOpenPod and our KnownProfiles:

| Format ID | iOpenPod Dims | Our Dims | iOpenPod Encoding | Our Encoding | Match |
|-----------|--------------|---------|-------------------|-------------|-------|
| 1005 | 80×80 | 80×80 | RGB565_LE | Rgb565 | ✅ |
| 1007 | 480×864 | 480×864 | RGB565_LE | Rgb565 | ✅ |
| 1009 | 42×30 | 42×30 | RGB565_LE | Rgb565 | ✅ |
| 1010 | 240×240 | 240×240 | RGB565_LE | Rgb565 | ✅ |
| 1013 | 220×176 | 220×176 | RGB565_BE_90 | Rgb565, LE:false, Rotation:90 | ✅ |
| 1015 | 130×88 | 130×88 | RGB565_LE | Rgb565 | ✅ |
| 1016 | 140×140 | 140×140 | RGB565_LE | Rgb565 | ✅ |
| 1017 | 56×56 | 56×56 | RGB565_LE | Rgb565 | ✅ |
| 1019 | 720×480 | 720×480 | UYVY | Yuv422 | ✅ |
| 1020 | 220×176 | 176×220† | RGB565_BE_90 | Rgb565, SwapsDim:true, LE:false | ✅† |
| 1023 | 176×132 | 176×132 | RGB565_BE | Rgb565, LE:false | ✅ |
| 1024 | 320×240 | 320×240 | RGB565_LE | Rgb565 | ✅ |
| 1027 | 100×100 | 100×100 | RGB565_LE | Rgb565 | ✅ |
| 1028 | 100×100 | 100×100 | RGB565_LE | Rgb565 | ✅ |
| 1029 | 200×200 | 200×200 | RGB565_LE | Rgb565 | ✅ |
| 1031 | 42×42 | 42×42 | RGB565_LE | Rgb565 | ✅ |
| 1032 | 42×37 | 42×37 | RGB565_LE | Rgb565 | ✅ |
| 1036 | 50×41 | 50×41 | RGB565_LE | Rgb565 | ✅ |
| 1044 | 128×128 | 128×128 | RGB565_LE | Rgb565 | ✅ |
| 1055 | 128×128 | 128×128 | RGB565_LE | Rgb565 | ✅ |
| 1056 | 128×128 | 128×128 | RGB565_LE | Rgb565 | ✅ |
| 1060 | 320×320 | 320×320 | RGB565_LE | Rgb565 | ✅ |
| 1066 | 64×64 | 64×64 | RGB565_LE | Rgb565 | ✅ |
| 1067 | 720×480 | 720×480 | I420_LE | Ycbcr420 | ✅ |
| 1068 | 128×128 | 128×128 | RGB565_LE | Rgb565 | ✅ |
| 1071 | 240×240 | 240×240 | RGB565_LE | Rgb565 | ✅ |
| 1073 | 240×240 | 240×240 | RGB565_LE | Rgb565 | ✅ |
| 1074 | 50×50 | 50×50 | RGB565_LE | Rgb565 | ✅ |
| 1078 | 80×80 | 80×80 | RGB565_LE | Rgb565 | ✅ |
| 1079 | 80×80 | 80×80 | RGB565_LE | Rgb565 | ✅ |
| 1083 | 240×320 | 240×320 | RGB565_LE | Rgb565 | ✅ |
| 1084 | 240×240 | 240×240 | RGB565_LE | Rgb565 | ✅ |
| 1085 | 88×88 | 88×88 | RGB565_LE | Rgb565 | ✅ |
| 1087 | 384×384 | 384×384 | RGB565_LE | Rgb565 | ✅ |
| 1089 | 58×58 | 58×58 | RGB565_LE | Rgb565 | ✅ |
| 1092 | 80×80 | 80×80 | RGB565_LE | Rgb565 | ✅ |
| 1093 | 512×512 | 512×512 | RGB565_LE | Rgb565 | ✅ |
| 2002 | 50×50 | 50×50 | RGB565_BE | Rgb565, LE:false | ✅ |
| 2003 | 150×150 | 150×150 | RGB565_BE | Rgb565, LE:false | ✅ |

† **1020 note:** iOpenPod stores 1020 as `220×176, RGB565_BE_90`. Our profile stores `Width=176, Height=220` with `SwapsDimensions: true`, which means after the swap we get 220×176. This is equivalent — the stored raster is 176×220 (rotated storage), decoded as 220×176. ✅

#### Profiles with Discrepancies

**1061 — 55×55 vs 56×56 (⚠️ Partial)**
- **iOpenPod:** `ArtworkFormat(1061, 56, 56, 112, "RGB565_LE", "cover_small")` — width=56, height=56, row_bytes=112
- **Our code:** `new(1061, 55, 55, IthmbEncoding.Rgb565, 56 * 55 * 2, UseMhniDimensions: true)`
- **Analysis:** iOpenPod says 56×56 with 112 row_bytes. Our profile says 55×55 visible with 56-pixel row stride (56×55×2 frame bytes). This follows the libgpod comment "officially 55×55 — verify!" and uses MHNI dimensions when available. This is a known ambiguity — iOpenPod/libusb choose 56×56, libgpod hints at 55×55. Our approach with `UseMhniDimensions` is a reasonable resolution, but differs from iOpenPod's hardcoded values.

**1081 — JPEG vs RGB565 (⚠️ Partial, documented)**
- **iOpenPod:** `ArtworkFormat(1081, 640, 480, 0, "JPEG", "photo_full")` — pixel_format=JPEG
- **Our code:** `new(1081, 640, 480, IthmbEncoding.Rgb565, 640 * 480 * 2)`
- **Analysis:** Our code has a detailed comment: "libgpod declares 1081 as THUMB_FORMAT_JPEG (unimplemented — no working JPEG decode in libgpod); iOpenPod's empirical testing indicates RGB565 640×480. Keeping RGB565 as primary since libgpod's JPEG path was never implemented." The iOpenPod format table says `row_bytes=0, pixel_format="JPEG"`, which is the experimental/legacy JPEG designation. Our code directly acknowledges this discrepancy and makes a deliberate choice. The iOpenPod source itself notes "(experimental/legacy)".

**3001/3002/3003 — REC_RGB555_LE vs ReorderedRgb555 (✅ Correct)**
- **iOpenPod:** These use `REC_RGB555_LE` pixel format. In `ithmb_codecs.py` lines 456-460, the encoding path for `REC_RGB555_LE` is **identical** to `RGB555_LE` — both call `_rgb555_array_from_image()` (standard RGB555 packing) and output as `"<u2"` (little-endian). Similarly, the decode path (line 582) handles `REC_RGB555_LE` identically to `RGB555_LE`. **iOpenPod's codec implementation is incomplete** — it treats REC_RGB555_LE as plain RGB555_LE without implementing the actual reordering.
- **Our code:** `IthmbEncoding.ReorderedRgb555` which applies a **Morton Z-order derangement** during encode and reorder during decode.
- **Empirical verification:** The iOpenPod creator confirmed on actual iPod Touch hardware that REC_RGB555_LE data is Morton Z-order reordered. Our implementation matches real device behavior.
- **Verdict:** ✅ Correct — Our ReorderedRgb555 matches the actual iPod Touch firmware behavior as empirically verified by the iOpenPod creator. iOpenPod's own source code is *incomplete* here — it only handles the base RGB555_LE path without the REC-required reordering.

**3005 — REC_RGB555 vs Rgb555**
- **iOpenPod:** `ArtworkFormat(3005, 320, 320, 640, "RGB555_LE", "cover_xlarge")` — pixel_format=RGB555_LE (not REC_)
- **Our code:** `new(3005, 320, 320, IthmbEncoding.Rgb555, 320 * 320 * 2)`
- **Analysis:** iOpenPod uses plain RGB555_LE for 3005. Our code uses Rgb555 (not ReorderedRgb555). ✅ Authoritative — we correctly handle 3005 as plain RGB555.

#### Missing Nano 7G Overrides (⚠️ Partial)

iOpenPod defines `NANO_7G_COVER_ART_OVERRIDES` in `artwork_presets.py` lines 95-101:

| Format ID | Global Def | Nano 7G Override |
|-----------|-----------|-----------------|
| 1013 | 220×176, RGB565_BE_90, photo_full | 50×50, RGB565_LE, cover_xsmall |
| 1015 | 130×88, RGB565_LE, photo_preview | 58×58, RGB565_LE, cover_small |
| 1016 | 140×140, RGB565_LE, cover_large | 57×57, RGB565_LE, cover_small_alt |

Our KnownProfiles only contain the global definitions for these IDs. For Nano 7G devices, this means our profiles would return the wrong dimensions. The overrides are **device-specific** and our flat KnownProfiles don't support device-contextual overrides. This is a known limitation of the flat-profile approach vs iOpenPod's layered resolution.

#### Profiles Not in iOpenPod's Registry

The following profiles in our KnownProfiles are **not present** in `ARTWORK_FORMATS_BY_ID`:

| Format ID | Origin (per our code) |
|-----------|----------------------|
| 1042 (320×240) | pygpod (libgpod binding) — ❓ Unverifiable from iOpenPod |
| 1043 (130×88) | pygpod (libgpod binding) — ❓ Unverifiable from iOpenPod |
| 3004 (56×55, padded, slot 8192) | libgpod itdb_device.c — ❓ Unverifiable from iOpenPod |
| 3006 (56×56, padded, slot 8192) | libgpod — ❓ Unverifiable from iOpenPod |
| 3007 (88×88, padded, slot 16384) | libgpod — ❓ Unverifiable from iOpenPod |
| 3008 (640×480) | libgpod — ❓ Unverifiable from iOpenPod |
| 3009 (160×120) | libgpod — ❓ Unverifiable from iOpenPod |
| 3011 (80×79) | libgpod — ❓ Unverifiable from iOpenPod |

All of these are honestly attributed to libgpod in our comments, so the claims are not false — they're just from a different source.

---

### 2. Device Capabilities: DeviceProfiles.cs vs capabilities.py

**Source (iOpenPod):** `/tmp/iOpenPod/ipod_device/capabilities.py` — `_FAMILY_GEN_CAPABILITIES` dict
**Our file:** `IthmbCodecPlugin.DeviceProfiles.cs` — `BuildDeviceProfiles()` method

**Overall Assessment: ⚠️ Partial — significant discrepancies across most device profiles.**

| Device | iOpenPod Formats | Our Formats | Verdict |
|--------|-----------------|-------------|---------|
| **Classic 5G (Video)** | photo: 1036, 1024, 1015, 1019; cover: 1028, 1029 | 1019, 1024, 1027, 1028, 1029, 1031, 1032 | ❌ Missing 1015, 1036; extra 1027, 1031, 1032 |
| **Classic 5.5G** | photo: 1036, 1024, 1015, 1019; cover: 1028, 1029 | same + 1055, 1056 | ❌ Same missing + 1056 description says "80×80" but iOpenPod says 128×128, alt cover |
| **Classic 6G** | photo: 1067, 1024, 1066; cover: 1055, 1060, 1061, 1068 | 1024, 1055, 1060, 1061, 1066, 1067, 1068 | ✅ Full match (format IDs) |
| **Nano 1G** | photo: 1032, 1023; cover: 1031, 1027 | 1024, 1027 | ❌ Missing 1023, 1031, 1032; has 1024 which iOpenPod doesn't list |
| **Nano 2G** | photo: 1032, 1023; cover: 1031, 1027 | 1019, 1027, 1028, 1029, 1032 | ❌ Extra 1019, 1028, 1029; missing 1023, 1031 |
| **Nano 3G** | photo: 1067, 1024, 1066; cover: 1061, 1055, 1068, 1060 | 1066, 1067, 1068, 1071, 1073, 1074 | ⚠️ Partial match (1066, 1067, 1068 correct); extra 1071, 1073, 1074; missing 1024, 1055, 1060, 1061 |
| **Nano 4G** | photo: 1024, 1066, 1079, 1083; cover: 1055, 1068, 1071, 1074, 1078, 1084 | 1071, 1073, 1074, 1078, 1079, 1083, 1084, 1085, 1087, 1089, 1092, 1093 | ⚠️ 6 correct, 2 missing (1024, 1055, 1066, 1068), 6 extra (1073, 1085, 1087, 1089, 1092, 1093) |
| **Nano 5G** | photo: 1087, 1079, 1066; cover: 1056, 1078, 1073, 1074 | 1087, 1092, 1093 | ❌ Only 1087 matches; missing 1056, 1066, 1073, 1074, 1078, 1079; has 1092, 1093 (Nano 6G formats) |
| **Nano 6G** | photo: 1092, 1093; cover: 1073, 1085, 1089, 1074 | 1084, 1092, 1093 | ⚠️ 1092, 1093 correct; missing 1073, 1074, 1085, 1089; has 1084 |
| **Nano 7G** | photo: 1007, 1005; cover: NANO_7G_OVERRIDES (1010, 1013@50, 1015@58, 1016@57) | 1007, 1010 | ❌ Only 1007 and 1010 present; missing all Nano 7G-specific override dimensions |
| **Video 5G** | Same as Classic 5G | 1019, 1024, 1027, 1028, 1029, 1031, 1032 | ❌ Same issues as Classic 5G |
| **Mini 1G/2G** | iOpenPod: `supports_artwork=False` — **no formats** | 1024, 1027 | ❌ iOpenPod says Mini does NOT support artwork at all. These formats should be removed. |
| **Photo 4G** | photo: 1009, 1013, 1015, 1019; cover: 1017, 1016 | 1013, 1015, 1016, 1019 | ⚠️ 4/6 correct; missing 1009, 1017 |
| **Touch 1G/2G** | Not in capabilities.py | 3001-3005, 3008, 3009, 3011 | ❓ Unverifiable from iOpenPod (no Touch capability entries) |
| **Touch 3G/4G** | Not in capabilities.py | Same as Touch 1G/2G | ❓ Unverifiable from iOpenPod |
| **iPhone 1G/2G** | Not in capabilities.py | Same as Touch | ❓ Unverifiable from iOpenPod |
| **iPhone 3G/3GS** | Not in capabilities.py | Same as Touch | ❓ Unverifiable from iOpenPod |
| **Motorola ROKR** | Global registry (2002, 2003) | 2002, 2003 | ✅ Authoritative |

**Key finding:** Several of our device profiles appear to have incorrect format ID assignments — mixing Nano 6G formats into Nano 4G/5G, including Mini formats when Mini has no artwork support, and missing photo_formats for many Video-era devices.

---

### 3. `_crop_visible_region` vs Our Crop Logic

**Source (iOpenPod):** `/tmp/iOpenPod/ArtworkDB_Writer/ithmb_codecs.py` — `_crop_visible_region()` (lines 372-416)
**Our file:** `IthmbCodecPlugin.DecodePipeline.cs` — crop section (lines 342-367)

**Comparison:**

| Aspect | iOpenPod _crop_visible_region | Our Crop System |
|--------|------------------------------|-----------------|
| Approach | Heuristic — computes crop from stored dimensions vs visible (w/h) + padding (hpad/vpad) | Explicit — uses profile fields CropX, CropY, CropWidth, CropHeight |
| Trigger | Auto-triggered for photo_role formats with non-zero hpad/vpad | Only when CropWidth > 0 && CropHeight > 0 |
| Rotation | Applied by caller before crop (RGB565_BE_90 → np.rot90) | Applied post-decode, then crop AFTER rotation |
| Padding logic | Centers crop for photo formats: `crop_x = min(max(0, hpad), max(0, stored_w - 1))` | Explicit offsets, no centering logic |
| Dependency | Requires format role from ArtworkFormat definition | Uses literal profile fields |

**Verdict: ⚠️ Partial** — The concept (cropping to the visible region after decode) is the same, but the implementation is fundamentally different:

- iOpenPod uses **dynamic/heuristic** cropping based on hpad/vpad and format role (photo_*)
- Our code uses **explicit per-profile** crop coordinates
- Our crop-after-rotation behavior is **not present** in iOpenPod — iOpenPod applies rotation in the codec layer (`RGB565_BE_90 → np.rot90`) before the crop is ever called, not as a post-decode rotation + then crop
- Our comment correctly says "Based on iOpenPod's `_crop_visible_region` approach (48-profile analysis)" — the inspiration is correct, but the implementation diverges

---

### 4. `_resolve_packed_geometry` vs TrailingPaddingTolerance

**Source (iOpenPod):** `/tmp/iOpenPod/ArtworkDB_Writer/ithmb_codecs.py` — `_resolve_packed_geometry()` lines 259-260
**Our file:** `IthmbCodecPlugin.cs` line 54 and `DecodePipeline.cs` lines 259-308

**iOpenPod's threshold (line 260):**
```python
if len(pixel_bytes) > expected_bytes and (len(pixel_bytes) - expected_bytes) <= 256:
```
The trailing-trim fallback in `_resolve_packed_geometry` only activates when:
1. Payload is **larger** than expected (extra bytes = alignment padding)
2. The excess is ≤ 256 bytes

**Our threshold (line 54):**
```csharp
private const int TrailingPaddingTolerance = 256;
```
We use the same numeric threshold of 256 bytes, but apply it differently:
- We allow payloads that are **smaller** (shorter) than expected, zero-padding to fill
- We also allow payloads slightly larger than expected (within tolerance)

**Verdict: ✅ Authoritative** — The 256-byte threshold value is identical. Our comment "Based on analysis of iOpenPod's _resolve_packed_geometry trailing-trim approach" correctly identifies the source. The application direction differs (we handle short payloads, iOpenPod handles long payloads) but this is a reasonable adaptation for decode vs geometry inference.

---

### 5. PackRgb565 Reference vs iOpenPod

**Source (iOpenPod):** `/tmp/iOpenPod/ArtworkDB_Writer/ithmb_codecs.py` — `_rgb565_array_from_image()` (lines 92-97)
**Our file:** `IthmbCodecTests.Helpers.cs` — `PackRgb565()` (lines 126-127)

**iOpenPod:**
```python
r = (arr[:, :, 0] >> 3) & 0x1F
g = (arr[:, :, 1] >> 2) & 0x3F
b = (arr[:, :, 2] >> 3) & 0x1F
return ((r << 11) | (g << 5) | b).astype(np.uint16)
```

**Our code:**
```csharp
private static ushort PackRgb565(int r, int g, int b) =>
    (ushort)(((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3));
```

**Analysis:** Identical bit operations:
- Red: bits 11-15, top 5 bits (>> 3)
- Green: bits 5-10, top 6 bits (>> 2)
- Blue: bits 0-4, top 5 bits (>> 3)
- iOpenPod uses `>> 3` without explicit masking for R and B, but since int32 has more bits, the Python `astype(np.uint16)` truncates. Our C# explicit cast to `ushort` does the same truncation.

**Verdict: ✅ Authoritative** — Bit packing is identical. The comment in our test "Reference RGB565 packer matching iOpenPod's Python _pack_rgb565" (which references `_pack_rgb565` — actually named `_rgb565_array_from_image` in the current iOpenPod code) is correctly describing the match.

---

### 6. RgbToYuv BT.601 Coefficients — CRITICAL MISMATCH

**Source (iOpenPod):** `/tmp/iOpenPod/ArtworkDB_Writer/ithmb_codecs.py` — UYVY encoder (lines 478-488)
**Our file:** `IthmbCodecTests.Helpers.cs` — `RgbToYuv()` (lines 130-137)
**Our encoder:** `IthmbCodecPlugin.EncoderHelpers.cs` — `Bt601Y/Bt601Cb/Bt601Cr` (lines 83-103)

**iOpenPod UYVY encoder forward transform:**
```python
y = np.clip(0.257 * r + 0.504 * g + 0.098 * b + 16, 0, 255).astype(np.uint8)
u = np.clip(-0.148 * r - 0.291 * g + 0.439 * b + 128, 0, 255)
v = np.clip(0.439 * r - 0.368 * g - 0.071 * b + 128, 0, 255)
```

**Our RgbToYuv test helper:**
```csharp
int y = (77 * r + 150 * g + 29 * b) >> 8;
int u = ((-43 * r - 85 * g + 128 * b) >> 8) + 128;
int v = ((128 * r - 107 * g - 21 * b) >> 8) + 128;
```

**Our Bt601Y/Bt601Cb/Bt601Cr encoder (EncoderHelpers.cs):**
```csharp
Bt601Y:  (77 * r + 150 * g + 29 * b) >> 8      // ≈ 0.301*R + 0.586*G + 0.113*B
Bt601Cb: ((-43 * r - 85 * g + 128 * b) >> 8) + 128  // ≈ -0.168*R - 0.332*G + 0.500*B + 128
Bt601Cr: ((128 * r - 107 * g - 21 * b) >> 8) + 128  // ≈ 0.500*R - 0.418*G - 0.082*B + 128
```

**Comparison:**

| Channel | iOpenPod | Our Code | Difference |
|---------|---------|----------|------------|
| Y (R coefficient) | 0.257 → 66/256 | 77/256 ≈ 0.301 | **Different** — iOpenPod = studio-range, we = full-range |
| Y (G coefficient) | 0.504 → 129/256 | 150/256 ≈ 0.586 | **Different** |
| Y (B coefficient) | 0.098 → 25/256 | 29/256 ≈ 0.113 | **Different** |
| Y offset | **+16** | **none** | **Different** — iOpenPod uses studio range (16-235), we use full range (0-255) |
| Cb (R coefficient) | -0.148 → -38/256 | -43/256 ≈ -0.168 | Different |
| Cb (G coefficient) | -0.291 → -74/256 | -85/256 ≈ -0.332 | Different |
| Cb (B coefficient) | 0.439 → 112/256 | 128/256 = 0.500 | Different |
| Cr (R coefficient) | 0.439 → 112/256 | 128/256 = 0.500 | Different |
| Cr (G coefficient) | -0.368 → -94/256 | -107/256 ≈ -0.418 | Different |
| Cr (B coefficient) | -0.071 → -18/256 | -21/256 ≈ -0.082 | Different |

**Verdict: ❌ Incorrect** — Our test's claim "Reference BT.601 forward transform matching iOpenPod's UYVY encoder" is false. The coefficients do not match:

- **iOpenPod** uses **BT.601 studio-range** (broadcast) coefficients with Y offset of 16, producing Y in [16,235]
- **Our code** uses **BT.601 full-range** (JPEG) coefficients with no Y offset, producing Y in [0,255]

These are two different well-known variants of BT.601. The test is wrong to claim it matches iOpenPod's encoder. The coefficients match our own encoder (which also uses full-range), but not iOpenPod's.

Our encoder/decoder pair is internally consistent (both use full-range BT.601), so the roundtrip is correct. But iOpenPod uses studio-range, meaning our encoder output would not decode correctly with iOpenPod's decoder, and vice versa.

**Impact:** The `RgbToYuv` test helper verifies our own encoder's behavior, not iOpenPod's. The comment is misleading.

---

### 7. PhotoDB/ArtworkDB Chunk Structure

**Source (iOpenPod):** 
- `/tmp/iOpenPod/ArtworkDB_Writer/artworkdb_chunks.py` — `_write_mhni`, `_write_mhii`, etc.
- `/tmp/iOpenPod/ArtworkDB_Shared/mhni.py` — `read_mhni_fields`, `MhniFields`
- `/tmp/iOpenPod/SyncEngine/photos.py` — PhotoDB chunk parsing

**Our file:** `PhotoDb/Core.cs`

**MHNI field layout comparison:**

| Offset | iOpenPod Field | Our Field |
|--------|--------------|----------|
| +0 | magic (b"mhni") | Magic |
| +4 | header_size (u32) | HeaderSize |
| +8 | total_len (u32) | (used for bounds) |
| +12 | child_count (u32) = 1 | — |
| +16 | format_id (u32) | FormatId (int) |
| +20 | ithmb_offset (u32) | IthmbOffset (int) |
| +24 | image_size (u32) | ImageSize (int) |
| +28 | vertical_padding (i16) | — (skipped) |
| +30 | horizontal_padding (i16) | — (skipped) |
| +32 | image_height (u16) | Height |
| +34 | image_width (u16) | Width |
| +40 | image_size_2 (u32) | — |

Our MhniHeader reads FormatId at +16, IthmbOffset at +20, ImageSize at +24, Width at +34 (u16), Height at +32 (u16). This matches iOpenPod's field layout exactly. We omit padding fields (HPadding/VPadding are set to 0).

**Chunk hierarchy comparison:**

| Level | iOpenPod (ArtworkDB) | iOpenPod (PhotoDB) | Our Code |
|-------|---------------------|--------------------|----------|
| Root | MHFD | MHFD | MHFD |
| Sections | MHSD | MHSD | MHSD |
| Lists | MHLI (cover), MHLA (albums), MHLF (files) | MHLI, MHLA, MHLF | MHLI (photos) |
| Items | MHII | MHII | MHII |
| Thumbnails | MHOD(type=8) → MHNI | MHOD(type=2) → MHNI | MHOD → MHNI |
| Filename | MHOD(type=3) in MHNI | MHOD(type=3) in MHNI | Not parsed |

**Verdict: ✅ Authoritative** — The chunk hierarchy and field layout match iOpenPod's implementation. Our code correctly identifies the MHNI fields at the right offsets. The comment in `Core.cs` explicitly references libgpod, iOpenPod, and Keith's iPod Photo Reader as sources.

---

### 8. Format 1013 Rotation Confirmation

**iOpenPod:** `ArtworkFormat(1013, 220, 176, 440, "RGB565_BE_90", "photo_full")`
- 220 × 176 pixels
- RGB565 big-endian with 90° rotation flag
- row_bytes = 440 (= 220 * 2)

**Our code:**
```csharp
[1013] = new(1013, 220, 176, IthmbEncoding.Rgb565, 220 * 176 * 2, LittleEndian: false, Rotation: 90)
```
- 220 × 176 pixels
- Big-endian (`LittleEndian: false`)
- 90° rotation

**Verdict: ✅ Authoritative** — Full match. Our comment references "per iOpenPod/libgpod" correctly. The big-endian + rotation interpretation matches iOpenPod's `RGB565_BE_90` pixel format exactly.

In iOpenPod's encoder (line 438):
```python
if pf == "RGB565_BE_90":
    rotated = base.transpose(Image.Transpose.ROTATE_270)
    arr16 = _rgb565_array_from_image(rotated)
    arr16 = _pad_packed_rows(arr16, stride)
    raw = arr16.astype(">u2").tobytes()
```
- **ROTATE_270** (not 90), but then stored as big-endian with rotated dimensions
- Our decoder: RotateBgra with Rotation=90 (clockwise) after decode, which is the inverse of ROTATE_270

This is consistent: iOpenPod rotates 270° **before** storage (transpose.ROTATE_270), and the stored data is at the rotated dimensions. We rotate 90° **after** decode (the reverse direction). The net effect is correct.

---

### 9. Format 1081 JPEG vs RGB565 Resolution

**iOpenPod:** `ArtworkFormat(1081, 640, 480, 0, "JPEG", "photo_full")`
- iOpenPod declares this as JPEG with row_bytes=0 (indicating variable-size payload)
- The role is "photo_full", which is for photo display, not cover art

**Our code:** `new(1081, 640, 480, IthmbEncoding.Rgb565, 640 * 480 * 2)`
- We treat it as RGB565 (640×480×2 = 614,400 bytes)

**Our comment (ProfileSystem.cs lines 62-63):**
```
// libgpod declares 1081 as THUMB_FORMAT_JPEG (unimplemented — no working JPEG 
// decode in libgpod); iOpenPod's empirical testing indicates RGB565 640×480.
```

**Analysis:** The comment accurately describes the situation:
- libgpod tables (which iOpenPod inherits) declare 1081 as JPEG
- iOpenPod's own empirical testing found RGB565 instead
- iOpenPod's `row_bytes=0` and `pixel_format="JPEG"` might be a legacy/compatibility entry

However, looking at iOpenPod's ARTWORK_FORMATS_BY_ID more carefully: `row_bytes=0` together with `pixel_format="JPEG"` makes it an experimental/legacy format — JPEGs have variable row size, so row_bytes=0 is expected for JPEG. But iOpenPod's own codec encoder handles JPEG (line 462-467) and decoder handles JPEG (line 718-722) for any format with `pf == "JPEG"`.

**Verdict: ⚠️ Partial** — Our code acknowledges the discrepancy and documents the rationale. The iOpenPod source data emphatically says JPEG, but our code (and the comment) claim iOpenPod's "empirical testing indicates RGB565". The raw iOpenPod source we have shows JPEG. This may be a discrepancy between iOpenPod's format table and its empirical findings, or the table is a legacy entry that wasn't updated.

---

### 10. Multi-Frame Concatenation

**iOpenPod evidence:** 
- `/tmp/iOpenPod/SyncEngine/photos.py` lines 1453-1458: Photo pipeline builds `payloads_by_format` as `bytearray()` and appends each photo's encoded data: `payloads_by_format[fmt_id].extend(info["data"])`
- The resulting F{fmt_id}_1.ithmb files contain concatenated frame data for multiple photos
- Individual photos reference their data via `ithmb_offset` and `size` in MHNI entries

**Our code:**
- `DecodePipeline.cs` lines 196-207: Multi-frame concatenation via frameIndex slicing: `frameStart = 4 + frameIndex * frameSize`
- Our mechanism assumes fixed-size frames within the concatenated file

**Verdict: ⚠️ Partial**

iOpenPod does concatenate ithmb payloads in the photo pipeline (multiple photos → single F{fmt_id}_1.ithmb), confirming the conceptual basis for multi-frame ithmb files. However:

1. iOpenPod's concatenation uses **offset + size** references via MHNI entries, not fixed frame sizes
2. Our approach assumes **equal-sized frames** (or slot boundaries for padded profiles)
3. The general concept is confirmed by iOpenPod, but the specific implementation differs

Our comment "Multi-frame concatenation confirmed by Keith's iPod Photo Reader, ithmbrdr, and iOpenPod" is conceptually correct — iOpenPod does create multi-frame ithmb files — but the way we slice frames differs from how iOpenPod reads them back.

---

### 11. PhotoDB Format ID Resolution in Core.cs

**Our code (Core.cs line 516-519):**
```
/// Format IDs map directly to KnownProfiles keys (e.g. 1019 → 720×480 Yuv422 interlaced).
/// The .ithmb data in PhotoDB/ArtworkDB is raw pixel data (no 4-byte F-prefix header).
```

**iOpenPod confirmation:** Correct — MHNI chunks store format_id directly as a uint32 at offset +16. The ithmb data blobs referenced by MHNI are raw pixel data without the 4-byte F-prefix header. The 4-byte prefix is only present in standalone F-prefix .ithmb files, not in data embedded within PhotoDB/ArtworkDB.

**Verdict: ✅ Authoritative**

---

### 12. Additional Observations

#### 12a. Interlaced 1019 — IsInterlaced flag

**Our code:** `[1019] = new(1019, 720, 480, IthmbEncoding.Yuv422, 720 * 480 * 2, IsInterlaced: true)`

**iOpenPod:** Defines 1019 as `UYVY` format with no explicit interlacing flag. However, iOpenPod has `_fix_1019_layout()` which handles the specific case of `format_id == 1019` by detecting stacked/half-resolution artifacts. This is iOpenPod's way of handling the same F1019 quirk, but through post-decode image analysis rather than field-interlace decoding.

iOpenPod's approach is different — it decodes the full UYVY data into RGB first, then checks if the image has field-split artifacts (half-height similarity), and reconstructs the full image by scaling or weaving fields. Our approach pre-splits the UYVY data into even/odd fields before decode.

Both approaches handle the same artifacts. The IsInterlaced flag is **not directly present** in iOpenPod's format definitions, but the handling of F1019's field-split characteristic is.

**Verdict: ⚠️ Partial** — The F1019 quirk handling is confirmed, but our IsInterlaced approach differs from iOpenPod's post-decode image analysis.

#### 12b. SwapChromaPlanes (profile field)

**Our code:** `SwapChromaPlanes` field in IthmbVariantProfile

**iOpenPod:** iOpenPod's `encode_image_for_format` for I420_LE doesn't have a swap-chroma option — it always outputs Y, then Cb, then Cr. There's no device-specific chroma plane reversal.

**Verdict: ❓ Unverifiable** — Not present in iOpenPod's source. This may come from other references or empirical findings.

#### 12c. CL/CLCL Chroma Formats

**Our code:** `ClChroma`, `ClclChroma` profile fields

**iOpenPod:** No CL or CLCL formats exist in `ARTWORK_FORMATS_BY_ID` or in the encode/decode logic. iOpenPod's codecs only handle: RGB565_LE/BE/BE_90, RGB555_LE/BE, REC_RGB555_LE, JPEG, UYVY, I420_LE.

**Verdict: ❓ Unverifiable** — CL/CLCL formats are not part of iOpenPod's codec set. These come from other sources (Keith's ithmbrdr, which documented Keith's CL/CLCL chroma packing methods).

---

## Reference Count Summary

| Verification | Count |
|-------------|-------|
|| ✅ Authoritative | 38+ (all format IDs with matching dims, PackRgb565, 1013 rotation, chunk structure, trailing tolerance, REC_RGB555_LE) |
| ⚠️ Partial | 12 (1061 ambiguity, 1081 encoding, crop concept, device profiles, multi-frame, interlaced 1019, Nano 7G overrides) |
|| ❌ Incorrect | 2 (RgbToYuv coefficients claim, Mini having artwork) |
| ❓ Unverifiable | 7+ (Touch/iPhone Touch profiles, CL/CLCL, SwapChromaPlanes, libgpod-only formats) |

---

## Recommendations

1. **Fix the RgbToYuv test comment** (Helpers.cs line 129): The test correctly tests our own encoder's full-range BT.601 coefficients, but the comment falsely claims they match iOpenPod. Either remove the "matching iOpenPod" claim or add a note explaining the studio-range vs full-range difference.

2. **REC_RGB555_LE treatment** (3001/3002/3003): ✅ Already correct — iOpenPod creator empirically verified Morton Z-order on actual hardware. iOpenPod's codec is incomplete.

3. **Cross-check DeviceProfiles systematically**: Several device profiles have significant format ID discrepancies with iOpenPod's capabilities.py (Nano 1G, Mini, Nano 5G, etc.). These should be audited against a fresh extraction of iOpenPod's device tables.

4. **Nano 7G overrides**: Add device-specific handling for Nano 7G's reinterpretation of IDs 1013/1015/1016, or document the limitation.

5. **Consider Nano 7G format IDs 1010/1005/1007**: Ensure those are the only formats needed for Nano 7G, or add the missing cover art variants.

6. **Crop system alignment**: Consider whether the explicit per-profile crop coordinates should be replaced or supplemented with iOpenPod's heuristic centering logic for photo formats.

7. **Mini device artwork**: Remove 1024 and 1027 from the Mini device profile, as iOpenPod clearly states Mini generations do not support artwork.
