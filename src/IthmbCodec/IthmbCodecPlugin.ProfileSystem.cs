// Known profile definitions and external profiles.json loader for .ithmb raw profiles.
// Provides the built-in 49-profile table and a minimal AOT-safe JSON parser for
// user-provided profile overrides.
// Separated from plugin ABI glue for independent AOT compilation.

using System.Collections.Frozen;
using System.IO;
using ImageGlass.SDK.Plugins;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    internal static volatile FrozenDictionary<int, IthmbVariantProfile> KnownProfiles = GetBuiltInProfiles();

    private static FrozenDictionary<int, IthmbVariantProfile> GetBuiltInProfiles() =>
        new Dictionary<int, IthmbVariantProfile>
        {
            [1007] = new(1007, 480, 864, IthmbEncoding.Rgb565, 480 * 864 * 2),
            // iPod Nano 7G full-resolution photo (from iOpenPod)
            [1005] = new(1005, 80, 80, IthmbEncoding.Rgb565, 80 * 80 * 2),
            [1009] = new(1009, 42, 30, IthmbEncoding.Rgb565, 42 * 30 * 2),
            // iPod Photo 4G full-screen (big-endian, per iOpenPod/libgpod)
            [1013] = new(1013, 220, 176, IthmbEncoding.Rgb565, 220 * 176 * 2, LittleEndian: false),
            [1015] = new(1015, 130, 88, IthmbEncoding.Rgb565, 130 * 88 * 2),
            // Nano 7G cover art large (iOpenPod)
            [1010] = new(1010, 240, 240, IthmbEncoding.Rgb565, 240 * 240 * 2),
            [1019] = new(1019, 720, 480, IthmbEncoding.Yuv422, 720 * 480 * 2, IsInterlaced: true),
            [1020] = new(1020, 176, 220, IthmbEncoding.Rgb565, 176 * 220 * 2, SwapsDimensions: true, LittleEndian: false),
            [1023] = new(1023, 176, 132, IthmbEncoding.Rgb565, 176 * 132 * 2, LittleEndian: false),
            // iPod Classic 5G/6G full-screen
            [1024] = new(1024, 320, 240, IthmbEncoding.Rgb565, 320 * 240 * 2),
            // Nano/Classic cover art variants (iOpenPod)
            [1027] = new(1027, 100, 100, IthmbEncoding.Rgb565, 100 * 100 * 2),
            // iPod Classic thumbnail
            [1031] = new(1031, 42, 42, IthmbEncoding.Rgb565, 42 * 42 * 2),  // Nano album art small
            // iPod Nano 1G/2G photo list thumbnail
            [1032] = new(1032, 42, 37, IthmbEncoding.Rgb565, 42 * 37 * 2),
            [1036] = new(1036, 50, 41, IthmbEncoding.Rgb565, 50 * 41 * 2),
            // iPod Nano 8GB (3G) photo library — 320×240 YCbCr 4:2:0, padded
            // Documented in Whirlpool forum thread (Anywho's iThmbConv, 2007).
            // No sample files exist for validation. Speculative — override via profiles.json if wrong.
            // [1064] speculated as 320×240 YCbCr420 padded — disabled: no real sample has ever
            // been found to validate. Not present in any known iPod Photo Cache dump, iOpenPod's
            // 50+ profiles, Keith's iPod Photo Reader, libgpod, etc. Re-enable only after
            // verifying against a real F1064 .ithmb file from actual hardware.
            // [1064] = new(1064, 320, 240, IthmbEncoding.Ycbcr420, 320 * 240 * 2, IsPadded: true),
            // iPod Classic 6G square photo thumbnail
            [1066] = new(1066, 64, 64, IthmbEncoding.Rgb565, 64 * 64 * 2),
            // iPod Classic 6G / nano 3G: 12-bit YCbCr 4:2:0 packed into 2 Bpp frame
            [1067] = new(1067, 720, 480, IthmbEncoding.Ycbcr420, 720 * 480 * 2, IsPadded: true),
            // Classic/Nano cover art variants (iOpenPod)
            [1068] = new(1068, 128, 128, IthmbEncoding.Rgb565, 128 * 128 * 2),
            // Nano 4G/5G/6G cover art variants (iOpenPod)
            [1071] = new(1071, 240, 240, IthmbEncoding.Rgb565, 240 * 240 * 2),
            [1073] = new(1073, 240, 240, IthmbEncoding.Rgb565, 240 * 240 * 2),
            [1074] = new(1074, 50, 50, IthmbEncoding.Rgb565, 50 * 50 * 2),
            [1078] = new(1078, 80, 80, IthmbEncoding.Rgb565, 80 * 80 * 2),
            // iPod Nano 4G photo thumbnails
            [1079] = new(1079, 80, 80, IthmbEncoding.Rgb565, 80 * 80 * 2),
            [1081] = new(1081, 640, 480, IthmbEncoding.Rgb565, 640 * 480 * 2),
            [1083] = new(1083, 240, 320, IthmbEncoding.Rgb565, 240 * 320 * 2),
            // Nano 4G/6G cover art variants (iOpenPod)
            [1084] = new(1084, 240, 240, IthmbEncoding.Rgb565, 240 * 240 * 2),
            [1085] = new(1085, 88, 88, IthmbEncoding.Rgb565, 88 * 88 * 2),
            [1089] = new(1089, 58, 58, IthmbEncoding.Rgb565, 58 * 58 * 2),
            // iPod Nano 5G photo
            [1087] = new(1087, 384, 384, IthmbEncoding.Rgb565, 384 * 384 * 2),
            // Cover art / album art formats (also stored as .ithmb, same RGB565 encoding)
            [1016] = new(1016, 140, 140, IthmbEncoding.Rgb565, 140 * 140 * 2),
            [1017] = new(1017, 56, 56, IthmbEncoding.Rgb565, 56 * 56 * 2),
            [1028] = new(1028, 100, 100, IthmbEncoding.Rgb565, 100 * 100 * 2),
            [1029] = new(1029, 200, 200, IthmbEncoding.Rgb565, 200 * 200 * 2),
            [1055] = new(1055, 128, 128, IthmbEncoding.Rgb565, 128 * 128 * 2),
            // Nano 5G cover art medium (iOpenPod)
            [1056] = new(1056, 128, 128, IthmbEncoding.Rgb565, 128 * 128 * 2),
            [1060] = new(1060, 320, 320, IthmbEncoding.Rgb565, 320 * 320 * 2),
            // Classic cover art small: 55×55 nominal, 56-pixel rows (Reuhno)
            [1061] = new(1061, 55, 55, IthmbEncoding.Rgb565, 56 * 55 * 2, UseMhniDimensions: true),
            // iPod Nano 6G photo thumbnail and full-screen
            [1092] = new(1092, 80, 80, IthmbEncoding.Rgb565, 80 * 80 * 2),
            [1093] = new(1093, 512, 512, IthmbEncoding.Rgb565, 512 * 512 * 2),

            // Compatibility alias for 1055 (same 128×128 cover art, older iTunes versions)
            [1044] = new(1044, 128, 128, IthmbEncoding.Rgb565, 128 * 128 * 2),

            // iPod Classic photo thumbnail aliases (pygpod photodb.py — likely identical to 1024/1015)
            [1042] = new(1042, 320, 240, IthmbEncoding.Rgb565, 320 * 240 * 2),
            [1043] = new(1043, 130, 88, IthmbEncoding.Rgb565, 130 * 88 * 2),

            // iPod Touch cover art with slot padding (libgpod itdb_device.c)
            // Each slot is padded to fixed byte boundaries regardless of image data size.
            [3006] = new(3006, 56, 56, IthmbEncoding.Rgb555, 56 * 56 * 2, IsPadded: true, SlotSize: 8192),
            [3007] = new(3007, 88, 88, IthmbEncoding.Rgb555, 88 * 88 * 2, IsPadded: true, SlotSize: 16384),

            // iPod Mobile (Motorola ROKR/SLVR/RAZR) cover art — big-endian (iOpenPod)
            [2002] = new(2002, 50, 50, IthmbEncoding.Rgb565, 50 * 50 * 2, LittleEndian: false),
            [2003] = new(2003, 150, 150, IthmbEncoding.Rgb565, 150 * 150 * 2, LittleEndian: false),
            // iPod touch cover art (iOpenPod) — RGB555 LE
            [3001] = new(3001, 256, 256, IthmbEncoding.ReorderedRgb555, 256 * 256 * 2),
            [3002] = new(3002, 128, 128, IthmbEncoding.ReorderedRgb555, 128 * 128 * 2),
            [3003] = new(3003, 64, 64, IthmbEncoding.ReorderedRgb555, 64 * 64 * 2),
            // iPod Touch / iPhone cover art — 320×320 square (libgpod itdb_device.c)
            [3005] = new(3005, 320, 320, IthmbEncoding.Rgb555, 320 * 320 * 2),
            // iPhone 1G/2G, iPod Touch 1G/2G photo thumbnail variants
            // Note: iOS 1.x used different dims per Steee29: 3004=55×55, 3009=120×160 (swapped!), 3011=75×75
            // Our dimensions are from libgpod (iOS 2G+). Use profiles.json to override if targeting iPhone 1.x.
            [3004] = new(3004, 56, 55, IthmbEncoding.Rgb555, 56 * 55 * 2, IsPadded: true, SlotSize: 8192),
            [3009] = new(3009, 160, 120, IthmbEncoding.Rgb555, 160 * 120 * 2),
            [3011] = new(3011, 80, 79, IthmbEncoding.Rgb555, 80 * 79 * 2),
            // iPhone 1G/2G, iPod Touch 1G/2G full-screen
            [3008] = new(3008, 640, 480, IthmbEncoding.Rgb555, 640 * 480 * 2),
        }.ToFrozenDictionary();

    // ------------------------------ External profiles.json ------------------------------

    /// <summary>
    /// Looks for a profiles.json sidecar file next to the plugin DLL.
    /// If found, parses it and merges entries into KnownProfiles (external overrides built-in).
    /// Safe for Native AOT: uses a minimal manual JSON parser (no reflection).
    /// </summary>
    private static void LoadExternalProfiles()
    {
        string? jsonPath = null;
        try
        {
            // Only load from app base directory (prevents working-directory injection)
            string baseDir = AppContext.BaseDirectory;
            jsonPath = System.IO.Path.Join(baseDir, "profiles.json");
            if (!File.Exists(jsonPath)) return;
        }
        catch (Exception) { return; }
        if (jsonPath == null) return;

        string json;
        try { json = File.ReadAllText(jsonPath); }
        catch (Exception) { return; }

        if (string.IsNullOrWhiteSpace(json)) return;

        var external = new Dictionary<int, IthmbVariantProfile>();
        try
        {
            ParseProfilesJson(json, external);
        }
        catch (Exception) { return; }

        if (external.Count == 0) return;

        // Merge: start with built-in, override with external, rebuild
        var merged = new Dictionary<int, IthmbVariantProfile>();
        foreach (var kv in GetBuiltInProfiles()) merged[kv.Key] = kv.Value;
        foreach (var kv in external) merged[kv.Key] = kv.Value;
        KnownProfiles = merged.ToFrozenDictionary();
    }

    /// <summary>Minimal AOT-safe JSON parser for the profiles.json schema.</summary>
    internal static void ParseProfilesJson(string json, Dictionary<int, IthmbVariantProfile> output)
    {
        int pos = 0;
        SkipWhitespace(json, ref pos);
        if (pos >= json.Length || json[pos] != '[') return;
        pos++; // skip '['

        int objectsRead = 0;
        bool foundEnd = false;
        while (pos < json.Length && !foundEnd)
        {
            SkipWhitespace(json, ref pos);
            if (pos >= json.Length) break;
            if (json[pos] == ']') { pos++; foundEnd = true; continue; }

            // Limit object count to prevent CPU DoS from crafted JSON
            if (objectsRead++ > 100) return;

            // Parse object
            if (json[pos] != '{') return;
            pos++; // skip '{'

            int prefix = 0, width = 0, height = 0, frameBytes = 0;
            string encoding = "rgb565";
            bool swapsDim = false, le = true, padded = false, interlaced = false, clcl = false, clSingle = false, swapPlanes = false, swapRgb = false;
            int rotationDeg = 0, cropX = 0, cropY = 0, cropW = 0, cropH = 0;
            bool useMhni = false;
            List<IthmbEncoding>? fallbackEncodings = null;

            while (pos < json.Length)
            {
                SkipWhitespace(json, ref pos);
                if (pos >= json.Length || json[pos] == '}') { pos++; break; }

                // Read key
                string? key = ParseJsonString(json, ref pos);
                if (key == null) return;
                SkipWhitespace(json, ref pos);
                if (pos >= json.Length || json[pos] != ':') return;
                pos++; // skip ':'
                SkipWhitespace(json, ref pos);

                // Read value (type depends on key)
                switch (key)
                {
                    case "prefix": prefix = ParseJsonInt(json, ref pos); break;
                    case "width": width = ParseJsonInt(json, ref pos); break;
                    case "height": height = ParseJsonInt(json, ref pos); break;
                    case "frameBytes": frameBytes = ParseJsonInt(json, ref pos); break;
                    case "encoding": encoding = ParseJsonString(json, ref pos) ?? "rgb565"; break;
                    case "swapsDimensions": swapsDim = ParseJsonBool(json, ref pos); break;
                    case "littleEndian": le = ParseJsonBool(json, ref pos); break;
                    case "isPadded": padded = ParseJsonBool(json, ref pos); break;
                    case "isInterlaced": interlaced = ParseJsonBool(json, ref pos); break;
                    case "isClcl": clcl = ParseJsonBool(json, ref pos); break;
                    case "isCl": clSingle = ParseJsonBool(json, ref pos); break;
                    case "swapChromaPlanes": swapPlanes = ParseJsonBool(json, ref pos); break;
                    case "swapRgbChannels": swapRgb = ParseJsonBool(json, ref pos); break;
                    case "rotation": rotationDeg = ParseJsonInt(json, ref pos); break;
                    case "cropX": cropX = ParseJsonInt(json, ref pos); break;
                    case "cropY": cropY = ParseJsonInt(json, ref pos); break;
                    case "cropWidth": cropW = ParseJsonInt(json, ref pos); break;
                    case "cropHeight": cropH = ParseJsonInt(json, ref pos); break;
                    case "useMhniDimensions": useMhni = ParseJsonBool(json, ref pos); break;
                    case "fallbackEncodings": fallbackEncodings = ParseEncodingArray(json, ref pos); break;
                    default: SkipJsonValue(json, ref pos, 32); break;
                }

                SkipWhitespace(json, ref pos);
                if (pos < json.Length && json[pos] == ',') pos++;
            }

            if (prefix > 0 && width > 0 && height > 0 && frameBytes > 0)
            {
                var enc = string.Equals(encoding, "yuv422", StringComparison.OrdinalIgnoreCase) ? IthmbEncoding.Yuv422
                    : string.Equals(encoding, "yuv422clcl", StringComparison.OrdinalIgnoreCase) ? (clcl = true, IthmbEncoding.Yuv422).Item2
                    : string.Equals(encoding, "yuv422cl", StringComparison.OrdinalIgnoreCase) ? (clSingle = true, IthmbEncoding.Yuv422).Item2
                    : string.Equals(encoding, "ycbcr420", StringComparison.OrdinalIgnoreCase) ? IthmbEncoding.Ycbcr420
                    : string.Equals(encoding, "rgb555", StringComparison.OrdinalIgnoreCase) ? IthmbEncoding.Rgb555
                    : string.Equals(encoding, "reorderedrgb555", StringComparison.OrdinalIgnoreCase) ? IthmbEncoding.ReorderedRgb555
                    : IthmbEncoding.Rgb565;
                output[prefix] = new IthmbVariantProfile(prefix, width, height, enc, frameBytes,
                    SwapsDimensions: swapsDim, LittleEndian: le, IsPadded: padded, IsInterlaced: interlaced, ClclChroma: clcl,
                    SwapChromaPlanes: swapPlanes, ClChroma: clSingle, SwapRgbChannels: swapRgb, Rotation: rotationDeg,
                    CropX: cropX, CropY: cropY, CropWidth: cropW, CropHeight: cropH,
                    UseMhniDimensions: useMhni, FallbackEncodings: fallbackEncodings?.ToArray());
            }

            SkipWhitespace(json, ref pos);
            if (pos < json.Length && json[pos] == ',') pos++;
        }
    }

    private static void SkipWhitespace(string s, ref int pos)
    {
        while (pos < s.Length)
        {
            char c = s[pos];
            if (c == ' ' || c == '\t' || c == '\n' || c == '\r') { pos++; continue; }
            // Skip // line comments (useful for profiles.json documentation)
            if (c == '/' && pos + 1 < s.Length && s[pos + 1] == '/')
            {
                while (pos < s.Length && s[pos] != '\n') pos++;
                continue;
            }
            break;
        }
    }

    private static string? ParseJsonString(string s, ref int pos)
    {
        if (pos >= s.Length || s[pos] != '"') return null;
        pos++; // skip opening quote
        int start = pos;
        while (pos < s.Length && s[pos] != '"')
        {
            if (s[pos] == '\\' && pos + 1 < s.Length) pos++; // skip escape sequence
            pos++;
        }
        if (pos >= s.Length) return null;
        string result = s[start..pos];
        pos++; // skip closing quote
        return result;
    }

    private static int ParseJsonInt(string s, ref int pos)
    {
        int sign = 1, val = 0;
        if (pos < s.Length && s[pos] == '-') { sign = -1; pos++; }
        while (pos < s.Length && s[pos] >= '0' && s[pos] <= '9')
        {
            int digit = s[pos] - '0';
            // Guard against integer overflow for malicious numeric strings
            if (val > (int.MaxValue - digit) / 10) { val = int.MaxValue; break; }
            val = val * 10 + digit;
            pos++;
        }
        return sign * val;
    }

    private static bool ParseJsonBool(string s, ref int pos)
    {
        if (pos + 4 <= s.Length && s[pos..(pos + 4)] == "true") { pos += 4; return true; }
        if (pos + 5 <= s.Length && s[pos..(pos + 5)] == "false") { pos += 5; return false; }
        return false; // default
    }

    private static void SkipJsonValue(string s, ref int pos, int maxDepth = 32)
    {
        if (pos >= s.Length) return;
        if (s[pos] == '"') { ParseJsonString(s, ref pos); return; }
        if (s[pos] == '{' || s[pos] == '[')
        {
            int depth = 1;
            pos++;
            while (pos < s.Length && depth > 0)
            {
                if (s[pos] == '{' || s[pos] == '[') depth++;
                else if (s[pos] == '}' || s[pos] == ']') depth--;
                pos++;
                // Prevent CPU DoS from deeply nested JSON
                if (depth > maxDepth) return;
            }
            return;
        }
        // null literal
        if (pos + 4 <= s.Length && s[pos] == 'n' && s[pos..(pos + 4)] == "null") { pos += 4; return; }
        // number or boolean
        while (pos < s.Length && s[pos] != ',' && s[pos] != '}' && s[pos] != ']' && !char.IsWhiteSpace(s[pos]))
            pos++;
    }

    /// <summary>Parses a JSON array of encoding strings, e.g. ["rgb555", "reorderedrgb555"].</summary>
    private static List<IthmbEncoding>? ParseEncodingArray(string s, ref int pos)
    {
        SkipWhitespace(s, ref pos);
        if (pos >= s.Length || s[pos] != '[') return null;
        pos++; // skip '['
        var list = new List<IthmbEncoding>();
        while (pos < s.Length)
        {
            SkipWhitespace(s, ref pos);
            if (pos >= s.Length) break;
            if (s[pos] == ']') { pos++; return list.Count > 0 ? list : null; }
            string? enc = ParseJsonString(s, ref pos);
            if (enc == null) break;
            list.Add(string.Equals(enc, "rgb565", StringComparison.OrdinalIgnoreCase) ? IthmbEncoding.Rgb565
                : string.Equals(enc, "rgb555", StringComparison.OrdinalIgnoreCase) ? IthmbEncoding.Rgb555
                : string.Equals(enc, "reorderedrgb555", StringComparison.OrdinalIgnoreCase) ? IthmbEncoding.ReorderedRgb555
                : string.Equals(enc, "yuv422", StringComparison.OrdinalIgnoreCase) ? IthmbEncoding.Yuv422
                : string.Equals(enc, "ycbcr420", StringComparison.OrdinalIgnoreCase) ? IthmbEncoding.Ycbcr420
                : IthmbEncoding.Rgb565);
            SkipWhitespace(s, ref pos);
            if (pos < s.Length && s[pos] == ',') pos++;
        }
        return null;
    }
}
