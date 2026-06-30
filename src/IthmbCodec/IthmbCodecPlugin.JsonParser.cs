// Minimal AOT-safe JSON parser for the profiles.json schema. Parses an array of
// profile objects into Dictionary<int, IthmbVariantProfile>. No reflection, no
// dependency on System.Text.Json. Separated from ProfileSystem for file-size discipline.

using ImageGlass.SDK.Plugins;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
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
            else
            {
                Log(4, $"ITHMB: profiles.json entry #{objectsRead} skipped — prefix={prefix}, width={width}, height={height}, frameBytes={frameBytes}");
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
        while (pos < s.Length && s[pos] != ',' && s[pos] != '}' && s[pos] != ']' && s[pos] != ' ' && s[pos] != '\t' && s[pos] != '\n' && s[pos] != '\r')
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
