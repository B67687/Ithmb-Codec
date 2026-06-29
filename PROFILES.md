# Profiles

55 known raw-format profiles (1 speculative profile disabled — see [F1064](#f1064-speculative-disabled)) covering iPod Photo 4G through iPhone 2G and iPod Nano 7G.

Additional profiles can be added at runtime via `profiles.json` without recompiling.

## Profile Table

| Profile | Resolution | Encoding         | Device(s)                                        |
| ------- | ---------- | ---------------- | ------------------------------------------------ |
| 1007    | 480×864    | RGB565           | iPod nano 7G (swapped dimensions)                |
| 1005    | 80×80      | RGB565           | iPod Nano 7G (photo thumbnail)                   |
| 1009    | 42×30      | RGB565           | iPod Photo 4G (smallest thumbnail)               |
| 1010    | 240×240    | RGB565           | Nano 7G (cover art large)                        |
| 1013    | 220×176    | RGB565 BE        | iPod Photo 4G (full-screen, big-endian)          |
| 1015    | 130×88     | RGB565           | iPod Photo 4G (slideshow browser)                |
| 1016    | 140×140    | RGB565           | iPod Photo 4G (cover art)                        |
| 1017    | 56×56      | RGB565           | iPod Photo 4G (cover art)                        |
| 1019    | 720×480    | UYVY (YUV 4:2:2) | iPod Photo/Video (TV-out, interlaced)            |
| 1020    | 176×220    | RGB565 BE        | iPod (portrait thumb, BE, rotated, swapped dims) |
| 1023    | 176×132    | RGB565 BE        | iPod Nano 1G/2G (landscape, big-endian)          |
| 1024    | 320×240    | RGB565           | iPod Classic 5G/6G (full-screen)                 |
| 1027    | 100×100    | RGB565           | Nano/Classic (cover art)                         |
| 1028    | 100×100    | RGB565           | iPod Video 5G (cover art)                        |
| 1029    | 200×200    | RGB565           | iPod Video 5G (cover art)                        |
| 1031    | 42×42      | RGB565           | iPod Nano (album art small)                      |
| 1032    | 42×37      | RGB565           | iPod Nano 1G/2G (photo list thumb)               |
| 1036    | 50×41      | RGB565           | iPod Classic (smallest thumbnail)                |
| 1055    | 128×128    | RGB565           | Classic/Nano3G/Nano4G (cover art)                |
| 1056    | 128×128    | RGB565           | Nano 5G (cover art)                              |
| 1060    | 320×320    | RGB565           | Classic/Nano3G (cover art)                       |
| 1042    | 320×240    | RGB565           | iPod Classic 5G/6G (photo alias for 1024)        |
| 1043    | 130×88     | RGB565           | iPod Photo 4G (alias for 1015)                   |
| 1044    | 128×128    | RGB565           | Compatibility alias for 1055                     |
| ~~1064~~ | ~~320×240~~ | ~~YCbCr 4:2:0~~  | ~~iPod Nano 8GB 3G (photo library, speculative — disabled, no real sample)~~ |
|| 1061    | 56×56      | RGB565           | Classic (cover art small, UseMhniDimensions=true — actual dims from MHNI)     |
| 1066    | 64×64      | RGB565           | iPod Classic 6G (square photo)                   |
| 1067    | 720×480    | YCbCr 4:2:0      | iPod Classic 6G / Nano 3G (padded)               |
| 1068    | 128×128    | RGB565           | Classic/Nano (cover art variant)                 |
| 1071    | 240×240    | RGB565           | Nano 4G (cover art large)                        |
| 1073    | 240×240    | RGB565           | Nano 5G/6G (cover art large)                     |
| 1074    | 50×50      | RGB565           | Nano 4G/5G/6G (cover art xsmall)                 |
| 1078    | 80×80      | RGB565           | Nano 4G/5G (cover art small)                     |
| 1079    | 80×80      | RGB565           | iPod Nano 4G (photo)                             |
| 1081    | 640×480    | RGB565           | iPod Classic/Nano (cover art large)              |
| 1083    | 240×320    | RGB565           | iPod Nano 4G (photo)                             |
| 1084    | 240×240    | RGB565           | Nano 4G (cover art alt)                          |
| 1085    | 88×88      | RGB565           | Nano 6G (cover art medium)                       |
| 1087    | 384×384    | RGB565           | iPod Nano 5G (photo)                             |
| 1089    | 58×58      | RGB565           | Nano 6G (cover art small)                        |
| 1092    | 80×80      | RGB565           | iPod Nano 6G (photo thumbnail)                   |
| 1093    | 512×512    | RGB565           | iPod Nano 6G (full-screen photo)                 |
| 2002    | 50×50      | RGB565 BE        | iPod Mobile / Motorola ROKR (cover art)          |
| 2003    | 150×150    | RGB565 BE        | iPod Mobile / Motorola ROKR (cover art)          |
|| 3001    | 256×256    | **Reordered RGB555** | iPod touch (cover art large, quad-tree Morton order)                         |
|| 3002    | 128×128    | **Reordered RGB555** | iPod touch (cover art medium, quad-tree Morton order)                        |
|| 3003    | 64×64      | **Reordered RGB555** | iPod touch (cover art small, quad-tree Morton order)                         |
|| 3004    | 56×55      | RGB555           | iPhone 1G/2G, iPod Touch (photo thumb, slot-padded 8192)                     |
| 3005    | 320×320    | RGB555           | iPod touch (cover art xlarge)                    |
| 3006    | 56×56      | RGB555           | iPod touch (cover art, padded slot 8192)          |
| 3007    | 88×88      | RGB555           | iPod touch (cover art, padded slot 16384)         |
|| 1062    | 56×56      | RGB565           | Nano 5G SysInfoExtended (clickwheel) — no device profile by default
| 3008    | 640×480    | RGB555           | iPhone 1G/2G, iPod Touch (full-screen)           |
|| 3009    | 120×160    | RGB555           | iPhone 1G/2G, iPod Touch (photo prev, portrait, padded)            
| 3011    | 80×79      | RGB555           | iPhone 1G/2G, iPod Touch (photo thumb)           |

> **Note:** iOS 1.x firmware used slightly different dimensions for some iPhone format IDs (e.g., 3004=55×55, 3009=120×160, 3011=75×75 per Steee29/ithmb_converter). Our dimensions target iPhone 2G+ (per libgpod). If your iOS 1.x files fail to decode, try adjusting the dimensions via `profiles.json`.
>
> The iLounge hacking thread (2005) and Whirlpool forum archive (2005–2009) document additional format IDs from community reverse-engineering. All known formats are covered by our 54 active profiles. **F1064** (320×240, iPod Nano 8GB) was previously included as a speculative YCbCr 4:2:0 padded profile based on Whirlpool thread analysis, but has been disabled — no real-world sample has ever been found across any surveyed implementation (iOpenPod, Keith's iPod Photo Reader, libgpod, ithmbrdr). If real samples emerge, re-enable via `profiles.json`.

> The codec parses TIFF IFD0 tag 0x0112 from the JPEG APP1 segment and sets orientation (1–8). ImageGlass uses this to auto-rotate.

## Advanced Profile Flags

These flags can be set in `profiles.json` for fine-tuning raw decoder behavior:

| Field              | Type | Default | Description                                                                                                       |
| ------------------ | ---- | ------- | ----------------------------------------------------------------------------------------------------------------- |
| `littleEndian`     | bool | `true`  | Byte order for 16-bit RGB (RGB565/RGB555). Set `false` for big-endian (iPod Photo 4G).                            |
| `swapsDimensions`  | bool | `false` | If true, swaps width and height (e.g., profile 1020 stores 220×176 but displays as 176×220 portrait).             |
| `isPadded`         | bool | `false` | Frame has trailing padding beyond valid pixel data (used by profile 1067 YCbCr 4:2:0).                            |
| `isInterlaced`     | bool | `false` | Even/odd rows stored separately (used by profile 1019 UYVY interlaced).                                           |
| `isClcl`           | bool | `false` | CLCL nibble-chroma: 4-bit chroma shared across 2 pixels (4 bytes per macropixel).                                 |
| `isCl`             | bool | `false` | CL per-pixel nibble-chroma: each pixel has its own 4-bit chroma (2 bytes per pixel). Keith's Methods 3/4.         |
| `swapRgbChannels`  | bool | `false` | When true, the RGB555 decoder reads `xBBBBBGGGGGRRRRR` (BGR15) instead of standard `xRRRRRGGGGGBBBBB`. For iPhone 2G thumbnail compatibility (Steee29/ithmb_converter). |
| `swapChromaPlanes` | bool | `false` | Swaps Cb/Cr order in YCbCr 4:2:0 planar decode (Keith's Method 6). For iPod variants with reversed chroma planes. |
| `cropX`            | int  | `0`     | X offset of visible region within decoded frame (0 = no crop). For centered-padding photo formats.                |
| `cropY`            | int  | `0`     | Y offset of visible region within decoded frame (0 = no crop).                                                    |
| `cropWidth`        | int  | `0`     | Width of visible region (0 = no crop, uses full frame width).                                                     |
| `cropHeight`       | int  | `0`     | Height of visible region (0 = no crop, uses full frame height).                                                   |
| `useMhniDimensions` | bool | `false` | When true, decode dimensions come from the MHNI chunk (format_id's Width/Height) instead of the profile's fixed Width/Height. Resolves real-world dimension disagreements between sources (e.g., 1061: libgpod=56×56, Reuhno=55×55).
| `fallbackEncodings`  | array | `null` | Ordered list of alternative encodings to try when the primary encoding fails to decode. Each entry must match one of the known encoding strings (e.g. `["rgb555", "reorderedrgb555"]`). Used when the exact encoding for a format ID is uncertain between sources.

> **Speculative decoders:** The `isClcl`, `isCl`, and `swapChromaPlanes` flags are based on Keith Wiley's original 2005 reverse engineering (Methods 1–6). No other open-source ithmb implementation (iOpenPod, libgpod, andrewmalta/ithmb, etc.) independently confirms these byte layouts or chroma orderings. No known built-in profiles use them — they are safety nets for undocumented iPod variants. If you encounter a file that decodes with wrong colors, try toggling these flags via `profiles.json`.
