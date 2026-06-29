// SPDX-License-Identifier: MIT
// Built-in .ithmb profile definitions as embedded JSON — single source of truth for
// all format dimensions and encoding parameters. Edit this file to change defaults;
// place a profiles.json next to the plugin DLL to override at runtime (no recompile).

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    /// <summary>
    /// Embedded JSON array of all 54 built-in profiles, parsed at module init by
    /// <see cref="GetBuiltInProfiles"/>. This replaces the previous hardcoded C# dictionary —
    /// dimensions are now data, not code. The schema matches what
    /// <see cref="ParseProfilesJson"/> expects (same format as external profiles.json).
    /// Entries are sorted by prefix for easy maintenance.
    /// </summary>
    private const string BuiltInProfilesJson = """
    [
      // iPod Nano 7G photo thumbnail
      {"prefix": 1005, "width": 80, "height": 80, "encoding": "rgb565", "frameBytes": 12800},
      // iPod Nano 7G full-resolution photo (from iOpenPod)
      {"prefix": 1007, "width": 480, "height": 864, "encoding": "rgb565", "frameBytes": 829440},
      // iPod Photo 4G smallest thumbnail
      {"prefix": 1009, "width": 42, "height": 30, "encoding": "rgb565", "frameBytes": 2520},
      // Nano 7G cover art large (iOpenPod)
      {"prefix": 1010, "width": 240, "height": 240, "encoding": "rgb565", "frameBytes": 115200},
      // iPod Photo 4G full-screen (big-endian, per iOpenPod/libgpod)
      // Nano 7G overrides this format to 50x50 RGB565 LE (cover art variant)
      {"prefix": 1013, "width": 220, "height": 176, "encoding": "rgb565", "frameBytes": 77440, "littleEndian": false, "rotation": 90},
      // Nano 7G overrides this format to 58x58 RGB565 LE (cover art variant)
      {"prefix": 1015, "width": 130, "height": 88, "encoding": "rgb565", "frameBytes": 22880},
      // Cover art / album art (Nano 7G overrides to 57x57 RGB565 LE)
      {"prefix": 1016, "width": 140, "height": 140, "encoding": "rgb565", "frameBytes": 39200},
      // iPod Photo 4G cover art
      {"prefix": 1017, "width": 56, "height": 56, "encoding": "rgb565", "frameBytes": 6272},
      // iPod Classic 5G/6G full-screen YUV422 interlaced
      {"prefix": 1019, "width": 720, "height": 480, "encoding": "yuv422", "frameBytes": 691200, "isInterlaced": true},
      // iPod Classic 3G/4G photo — rotated 90, swapped dimensions (portrait)
      {"prefix": 1020, "width": 176, "height": 220, "encoding": "rgb565", "frameBytes": 77440, "swapsDimensions": true, "littleEndian": false},
      // iPod Nano 1G/2G landscape (big-endian)
      {"prefix": 1023, "width": 176, "height": 132, "encoding": "rgb565", "frameBytes": 46464, "littleEndian": false},
      // iPod Classic 5G/6G full-screen
      {"prefix": 1024, "width": 320, "height": 240, "encoding": "rgb565", "frameBytes": 153600},
      // Nano/Classic cover art
      {"prefix": 1027, "width": 100, "height": 100, "encoding": "rgb565", "frameBytes": 20000},
      // iPod Video 5G cover art
      {"prefix": 1028, "width": 100, "height": 100, "encoding": "rgb565", "frameBytes": 20000},
      // iPod Video 5G cover art large
      {"prefix": 1029, "width": 200, "height": 200, "encoding": "rgb565", "frameBytes": 80000},
      // iPod Classic thumbnail / Nano album art small
      {"prefix": 1031, "width": 42, "height": 42, "encoding": "rgb565", "frameBytes": 3528},
      // iPod Nano 1G/2G photo list thumbnail
      {"prefix": 1032, "width": 42, "height": 37, "encoding": "rgb565", "frameBytes": 3108},
      // iPod Classic smallest thumbnail
      {"prefix": 1036, "width": 50, "height": 41, "encoding": "rgb565", "frameBytes": 4100},
      // iPod Classic photo thumbnail alias (matches 1024)
      {"prefix": 1042, "width": 320, "height": 240, "encoding": "rgb565", "frameBytes": 153600},
      // iPod Classic photo thumbnail alias (matches 1015)
      {"prefix": 1043, "width": 130, "height": 88, "encoding": "rgb565", "frameBytes": 22880},
      // iOpenPod #81: writing 1044 to iPod Classic corrupts cover art; disabled until confirmed safe.
      // {"prefix": 1044, "width": 128, "height": 128, "encoding": "rgb565", "frameBytes": 32768},
      {"prefix": 1055, "width": 128, "height": 128, "encoding": "rgb565", "frameBytes": 32768},
      // Nano 5G cover art medium (iOpenPod)
      {"prefix": 1056, "width": 128, "height": 128, "encoding": "rgb565", "frameBytes": 32768},
      // Classic/Nano3G cover art large
      {"prefix": 1060, "width": 320, "height": 320, "encoding": "rgb565", "frameBytes": 204800},
      // Classic cover art small: 55x55 nominal, 56-pixel rows (Reuhno)
      {"prefix": 1061, "width": 55, "height": 55, "encoding": "rgb565", "frameBytes": 6160, "useMhniDimensions": true},
      // clickwheel (dstaley) Nano 5G SysInfoExtended: 56x56 variant
      {"prefix": 1062, "width": 56, "height": 56, "encoding": "rgb565", "frameBytes": 6272},
      // iPod Classic 6G square photo thumbnail
      {"prefix": 1066, "width": 64, "height": 64, "encoding": "rgb565", "frameBytes": 8192},
      // iPod Classic 6G / Nano 3G: 12-bit YCbCr 4:2:0 packed into 2 Bpp frame
      {"prefix": 1067, "width": 720, "height": 480, "encoding": "ycbcr420", "frameBytes": 691200, "isPadded": true},
      // Classic/Nano cover art variant
      {"prefix": 1068, "width": 128, "height": 128, "encoding": "rgb565", "frameBytes": 32768},
      // Nano 4G cover art large
      {"prefix": 1071, "width": 240, "height": 240, "encoding": "rgb565", "frameBytes": 115200},
      // Nano 5G/6G cover art large
      {"prefix": 1073, "width": 240, "height": 240, "encoding": "rgb565", "frameBytes": 115200},
      // Nano 4G/5G/6G cover art xsmall
      {"prefix": 1074, "width": 50, "height": 50, "encoding": "rgb565", "frameBytes": 5000},
      // Nano 4G/5G cover art small
      {"prefix": 1078, "width": 80, "height": 80, "encoding": "rgb565", "frameBytes": 12800},
      // iPod Nano 4G photo thumbnail
      {"prefix": 1079, "width": 80, "height": 80, "encoding": "rgb565", "frameBytes": 12800},
      // libgpod declares 1081 as THUMB_FORMAT_JPEG; iOpenPod indicates RGB565 640x480.
      // If RGB565 decode fails, fallbackEncodings:["jpeg"] attempts JPEG decode.
      // No real JPEG 1081 sample has been found — this is a defensive fallback.
      // libgpod declares 1081 as THUMB_FORMAT_JPEG; iOpenPod indicates RGB565 640x480.
      // If RGB565 decode fails, fallbackEncodings:["jpeg"] attempts JPEG decode.
      // No real JPEG 1081 sample has been found — this is a defensive fallback.
      {"prefix": 1081, "width": 640, "height": 480, "encoding": "rgb565", "frameBytes": 614400, "fallbackEncodings": ["jpeg"]},
      {"prefix": 1083, "width": 240, "height": 320, "encoding": "rgb565", "frameBytes": 153600},
      // Nano 4G cover art alt
      {"prefix": 1084, "width": 240, "height": 240, "encoding": "rgb565", "frameBytes": 115200},
      // Nano 6G cover art medium
      {"prefix": 1085, "width": 88, "height": 88, "encoding": "rgb565", "frameBytes": 15488},
      // iPod Nano 5G photo
      {"prefix": 1087, "width": 384, "height": 384, "encoding": "rgb565", "frameBytes": 294912},
      // Nano 6G cover art small
      {"prefix": 1089, "width": 58, "height": 58, "encoding": "rgb565", "frameBytes": 6728},
      // iPod Nano 6G photo thumbnail
      {"prefix": 1092, "width": 80, "height": 80, "encoding": "rgb565", "frameBytes": 12800},
      // iPod Nano 6G full-screen photo
      {"prefix": 1093, "width": 512, "height": 512, "encoding": "rgb565", "frameBytes": 524288},
      // iPod Mobile (Motorola ROKR/SLVR/RAZR) cover art — big-endian (iOpenPod)
      {"prefix": 2002, "width": 50, "height": 50, "encoding": "rgb565", "frameBytes": 5000, "littleEndian": false},
      {"prefix": 2003, "width": 150, "height": 150, "encoding": "rgb565", "frameBytes": 45000, "littleEndian": false},
      // iPod touch cover art (iOpenPod) — RGB555 LE
      {"prefix": 3001, "width": 256, "height": 256, "encoding": "reorderedrgb555", "frameBytes": 131072},
      {"prefix": 3002, "width": 128, "height": 128, "encoding": "reorderedrgb555", "frameBytes": 32768},
      {"prefix": 3003, "width": 64, "height": 64, "encoding": "reorderedrgb555", "frameBytes": 8192},
      // iPhone 1G/2G, iPod Touch 1G/2G photo thumbnail — 56x55 slot padded to 8192
      // Steee29 (iPhone 2G iOS 1.1.4): content rect 55x55 inside 56x55 slot
      {"prefix": 3004, "width": 56, "height": 55, "encoding": "rgb555", "frameBytes": 6160, "isPadded": true, "slotSize": 8192},
      // iPod Touch / iPhone cover art — 320x320 square (libgpod itdb_device.c)
      {"prefix": 3005, "width": 320, "height": 320, "encoding": "rgb555", "frameBytes": 204800},
      // iPod Touch cover art with slot padding (libgpod itdb_device.c)
      {"prefix": 3006, "width": 56, "height": 56, "encoding": "rgb555", "frameBytes": 6272, "isPadded": true, "slotSize": 8192},
      {"prefix": 3007, "width": 88, "height": 88, "encoding": "rgb555", "frameBytes": 15488, "isPadded": true, "slotSize": 16384},
      // iPhone 1G/2G, iPod Touch 1G/2G full-screen
      {"prefix": 3008, "width": 640, "height": 480, "encoding": "rgb555", "frameBytes": 614400},
      // iPhone 1G/2G, iPod Touch 1G/2G photo thumbnail (3009) — 120x160 portrait slot padded to 40960
      // Steee29 (iPhone 2G iOS 1.1.4): 120x160 portrait, slotSize=40960
      {"prefix": 3009, "width": 120, "height": 160, "encoding": "rgb555", "frameBytes": 38400, "isPadded": true, "slotSize": 40960},
      // iPhone 1G/2G, iPod Touch 1G/2G cover art tiny (3011) — 80x79 slot
      // Steee29 (iPhone 2G iOS 1.1.4): content rect 75x75 inside 80x79 slot
      {"prefix": 3011, "width": 80, "height": 79, "encoding": "rgb555", "frameBytes": 12640}
    """;
}
