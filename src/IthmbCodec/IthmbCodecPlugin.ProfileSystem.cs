// Profile resolution system and external profiles.json loader for .ithmb raw profiles.
// Built-in profile definitions live in IthmbCodecPlugin.ProfilesJson.cs (embedded JSON).
// External profiles.json overrides take precedence at runtime — no recompile needed.

using System.Collections.Frozen;
using System.IO;
using ImageGlass.SDK.Plugins;
using System.Threading;

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
        catch (Exception ex) { Log(4, $"ITHMB: profiles.json path error: {ex.Message}"); return; }
        if (jsonPath == null) return;

        string json;
        try { json = File.ReadAllText(jsonPath); }
        catch (Exception ex) { Log(4, $"ITHMB: profiles.json read error: {ex.Message}"); return; }

        if (string.IsNullOrWhiteSpace(json)) return;

        var external = new Dictionary<int, IthmbVariantProfile>();
        try
        {
            ParseProfilesJson(json, external);
        }
        catch (Exception ex) { Log(4, $"ITHMB: profiles.json parse error: {ex.Message}"); return; }

        // FNV-1a hash of raw JSON for diagnostic traceability
        ulong hash = 14695981039346656037UL;
        foreach (byte b in System.Text.Encoding.UTF8.GetBytes(json))
            hash = (hash ^ b) * 1099511628211UL;
        Log(4, $"ITHMB: loaded {external.Count} profiles (FNV: {hash:X16})");

        // SHA-256 hash for integrity verification (log-only for v1.x)
        // The expected hash can be pinned once the canonical profiles.json is determined.
        string sha256Hex;
        using (var sha256 = System.Security.Cryptography.SHA256.Create())
        {
            byte[] hashBytes = sha256.ComputeHash(System.Text.Encoding.UTF8.GetBytes(json));
            sha256Hex = Convert.ToHexString(hashBytes).ToLowerInvariant();
        }
        Log(4, $"ITHMB: profiles.json SHA-256: {sha256Hex}");

        if (external.Count == 0) return;

        // Merge: start with built-in, override with external, rebuild
        var merged = new Dictionary<int, IthmbVariantProfile>();
        foreach (var kv in GetBuiltInProfiles()) merged[kv.Key] = kv.Value;
        foreach (var kv in external) merged[kv.Key] = kv.Value;
        Interlocked.Exchange(ref KnownProfiles, merged.ToFrozenDictionary());
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

}
