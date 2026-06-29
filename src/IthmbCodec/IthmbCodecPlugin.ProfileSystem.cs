// Profile resolution system and external profiles.json loader for .ithmb raw profiles.
// Built-in profile definitions live in IthmbCodecPlugin.ProfilesJson.cs (embedded JSON).
// External profiles.json overrides take precedence at runtime — no recompile needed.

using System.Collections.Frozen;
using System.IO;
using ImageGlass.SDK.Plugins;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    internal static volatile FrozenDictionary<int, IthmbVariantProfile> KnownProfiles = GetBuiltInProfiles();

    /// <summary>Parses the embedded <see cref="BuiltInProfilesJson">JSON</see> into the profile dictionary.</summary>
    private static FrozenDictionary<int, IthmbVariantProfile> GetBuiltInProfiles()
    {
        var dict = new Dictionary<int, IthmbVariantProfile>();
        ParseProfilesJson(BuiltInProfilesJson, dict);
        return dict.ToFrozenDictionary();
    }


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

    // ------------------------------ Device-contextual profile resolution ------------------------------

    /// <summary>
    /// Alternate profiles for format IDs with device-specific dimension/encoding variations.
    /// Keyed by format ID. Used by <see cref="ResolveProfile"/> to heuristically select
    /// the best match based on actual data size when device context is unavailable.
    /// </summary>
    /// <summary>Shared Nano 7G cover art overrides: global format IDs repurposed with smaller cover-art dimensions.</summary>
    private static readonly FrozenDictionary<int, IthmbVariantProfile> Nano7GOverrides = BuildNano7GOverrides();

    private static FrozenDictionary<int, IthmbVariantProfile> BuildNano7GOverrides()
    {
        return new Dictionary<int, IthmbVariantProfile>
        {
            [1013] = new(1013, 50, 50, IthmbEncoding.Rgb565, 50 * 50 * 2),
            [1015] = new(1015, 58, 58, IthmbEncoding.Rgb565, 58 * 58 * 2),
            [1016] = new(1016, 57, 57, IthmbEncoding.Rgb565, 57 * 57 * 2),
        }.ToFrozenDictionary();
    }

    private static volatile FrozenDictionary<int, IthmbVariantProfile[]> _profileAlternates
        = BuildProfileAlternates();

    /// <summary>
    /// Device-specific format overrides: device name → format ID → override profile.
    /// Used when the caller provides device context (e.g., from DeviceProfiles).
    /// Currently populated with Nano 7G overrides (global IDs repurposed with cover-art dimensions).
    /// </summary>
    private static volatile FrozenDictionary<string, FrozenDictionary<int, IthmbVariantProfile>> _deviceOverrides
        = BuildDeviceOverrides();

    /// <summary>
    /// Resolves the best-matching profile for a given format ID and raw pixel data.
    /// Resolution order:
    ///   1. Device-specific override (highest priority, requires device name context)
    ///   2. Data-size heuristic (matches alternate profiles by frame size vs data length)
    ///   3. Global KnownProfiles (fallback)
    /// </summary>
    /// <param name="formatId">The format ID from the .ithmb prefix or PhotoDB entry.</param>
    /// <param name="data">Raw pixel data (without 4-byte prefix). Used for size heuristic.</param>
    /// <param name="profile">The resolved profile when return is true.</param>
    /// <param name="deviceName">Optional device name for device-specific override lookup.</param>
    /// <returns>true if a profile was resolved; false if no profile matches the format ID.</returns>
    internal static bool TryResolveProfile(int formatId, ReadOnlySpan<byte> data, out IthmbVariantProfile profile, string? deviceName = null)
    {
        // 1. Device-specific override (highest priority, requires device context)
        if (deviceName != null
            && _deviceOverrides.TryGetValue(deviceName, out var deviceFormats)
            && deviceFormats.TryGetValue(formatId, out var deviceProfile))
        {
            profile = deviceProfile;
            return true;
        }

        // 2. Data-size heuristic: try to find the matching alternate profile
        if (_profileAlternates.TryGetValue(formatId, out var alternates))
        {
            int dataLen = data.Length;
            foreach (var alt in alternates)
            {
                if (alt.FrameByteLength > 0
                    && Math.Abs(dataLen - alt.FrameByteLength) <= TrailingPaddingTolerance)
                {
                    profile = alt;
                    return true;
                }
            }
        }

        // 3. Fall back to global KnownProfiles
        if (KnownProfiles.TryGetValue(formatId, out var globalProfile))
        {
            profile = globalProfile;
            return true;
        }

        profile = default;
        return false;
    }



    /// <summary>Builds the alternate profiles table for format IDs with device-specific variations.</summary>
    private static FrozenDictionary<int, IthmbVariantProfile[]> BuildProfileAlternates()
    {
        var alternates = new Dictionary<int, List<IthmbVariantProfile>>();

        // Helper: add an alternate profile for a format ID
        void AddAlt(int formatId, int w, int h, IthmbEncoding enc, int frameBytes,
            bool le = true, int rotation = 0)
        {
            if (!alternates.TryGetValue(formatId, out var list))
                alternates[formatId] = list = new();
            list.Add(new IthmbVariantProfile(formatId, w, h, enc, frameBytes,
                LittleEndian: le, Rotation: rotation));
        }

        // Nano 7G overrides (from shared Nano7GOverrides — same data powers BuildDeviceOverrides)
        foreach (var (_, alt) in Nano7GOverrides)
            AddAlt(alt.Prefix, alt.Width, alt.Height, alt.Encoding, alt.FrameByteLength);

        return alternates.ToFrozenDictionary(kv => kv.Key, kv => kv.Value.ToArray());
    }

    /// <summary>
    /// Used when device context is provided to <see cref="TryResolveProfile"/>
    /// (future: device selector, file metadata, or caller-provided hint).
    /// </summary>
    private static FrozenDictionary<string, FrozenDictionary<int, IthmbVariantProfile>> BuildDeviceOverrides()
    {
        return new Dictionary<string, FrozenDictionary<int, IthmbVariantProfile>>
        {
            ["iPod Nano 7G"] = Nano7GOverrides,
        }.ToFrozenDictionary();
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

            int prefix = 0, width = 0, height = 0, frameBytes = 0, slotSize = 0;
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
                    case "slotSize": slotSize = ParseJsonInt(json, ref pos); break;
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
                    CropX: cropX, CropY: cropY, CropWidth: cropW, CropHeight: cropH, SlotSize: slotSize,
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
                : string.Equals(enc, "jpeg", StringComparison.OrdinalIgnoreCase) ? IthmbEncoding.Jpeg
                : IthmbEncoding.Rgb565);
            SkipWhitespace(s, ref pos);
            if (pos < s.Length && s[pos] == ',') pos++;
        }
        return null;
    }
}
