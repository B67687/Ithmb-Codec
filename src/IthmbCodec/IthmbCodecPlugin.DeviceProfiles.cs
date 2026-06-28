// Per-generation iPod format ID tables — maps each known iPod/iPhone generation to the
// format IDs its thumbnail caches (PhotoDB & ArtworkDB) generate. Dimensions and encoding
// live in IthmbCodecPlugin.ProfilesJson.cs (embedded JSON) — the descriptions here only
// indicate each format's role on the device.
//
// Note: Some devices reinterpret global format IDs with different dimensions.
// Nano 7G overrides 1013, 1015, 1016 for cover art (see BuildDeviceOverrides).

using System.Collections.Frozen;
using System.Collections.Generic;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    /// <summary>Describes a single format a device generates, matching a KnownProfiles entry.</summary>
    internal sealed record DeviceFormatInfo(int FormatId, string Description);

    /// <summary>Groups the format IDs a specific device generation uses.</summary>
    internal sealed record DeviceProfile(string Name, DeviceFormatInfo[] Formats);

    /// <summary>Read-only map of device name to its profile, built once at module load.</summary>
    internal static readonly FrozenDictionary<string, DeviceProfile> DeviceProfiles = BuildDeviceProfiles();

    private static FrozenDictionary<string, DeviceProfile> BuildDeviceProfiles()
    {
        var dict = new Dictionary<string, DeviceProfile>
        {
            ["iPod Classic 5G (Video)"] = new("iPod Classic 5G (Video)",
            [
                new(1019, "Full-screen video"),
                new(1024, "Full-screen photo"),
                new(1027, "Cover art"),
                new(1028, "Thumbnail"),
                new(1029, "Thumbnail large"),
                new(1031, "Album art small"),
                new(1032, "Photo list thumbnail"),
            ]),

            ["iPod Classic 5.5G (Enhanced)"] = new("iPod Classic 5.5G (Enhanced)",
            [
                new(1019, "Full-screen video"),
                new(1024, "Full-screen photo"),
                new(1027, "Cover art"),
                new(1028, "Thumbnail"),
                new(1029, "Thumbnail large"),
                new(1031, "Album art small"),
                new(1032, "Photo list thumbnail"),
                new(1055, "Cover art medium"),
                new(1056, "Thumbnail"),
            ]),

            ["iPod Classic 6G (Thin)"] = new("iPod Classic 6G (Thin)",
            [
                new(1024, "Photo"),
                new(1055, "Cover art"),
                new(1060, "Cover art large"),
                new(1061, "Cover art small"),
                new(1066, "Photo thumbnail"),
                new(1067, "Full-screen video"),
                new(1068, "Cover art"),
            ]),

            ["iPod Nano 1G"] = new("iPod Nano 1G",
            [
                new(1024, "Photo"),
                new(1027, "Cover art"),
                new(1031, "Album art small"),
            ]),

            ["iPod Nano 2G"] = new("iPod Nano 2G",
            [
                new(1019, "Full-screen video"),
                new(1027, "Cover art"),
                new(1028, "Thumbnail"),
                new(1029, "Thumbnail large"),
                new(1032, "Photo list thumbnail"),
                new(1031, "Album art small"),
            ]),

            ["iPod Nano 3G"] = new("iPod Nano 3G",
            [
                new(1066, "Photo thumbnail"),
                new(1067, "Full-screen video"),
                new(1068, "Cover art"),
                new(1071, "Cover art"),
                new(1073, "Cover art"),
                new(1074, "Thumbnail"),
                new(1060, "Cover art large"),
                new(1055, "Cover art"),
                new(1061, "Cover art small"),
            ]),

            ["iPod Nano 4G"] = new("iPod Nano 4G",
            [
                new(1024, "Photo"),
                new(1055, "Cover art"),
                new(1066, "Photo thumbnail"),
                new(1068, "Cover art"),
                new(1071, "Cover art"),
                new(1074, "Thumbnail"),
                new(1078, "Photo thumbnail"),
                new(1079, "Photo thumbnail"),
                new(1083, "Photo"),
                new(1084, "Cover art"),
            ]),

            ["iPod Nano 5G"] = new("iPod Nano 5G",
            [
                new(1056, "Cover art medium"),
                new(1066, "Photo thumbnail"),
                new(1073, "Cover art"),
                new(1074, "Thumbnail"),
                new(1078, "Photo thumbnail"),
                new(1079, "Photo thumbnail"),
                new(1087, "Photo"),
            ]),

            ["iPod Nano 6G"] = new("iPod Nano 6G",
            [
                new(1073, "Cover art"),
                new(1074, "Thumbnail"),
                new(1085, "Thumbnail"),
                new(1089, "Thumbnail"),
                new(1092, "Photo thumbnail"),
                new(1093, "Photo large"),
            ]),

            ["iPod Nano 7G"] = new("iPod Nano 7G",
            [
                new(1007, "Full-res photo"),
                new(1010, "Cover art"),
                new(1013, "Cover art (Nano 7G override)"),
                new(1015, "Cover art (Nano 7G override)"),
                new(1016, "Cover art (Nano 7G override)"),
            ]),

            ["iPod Video 5G"] = new("iPod Video 5G",
            [
                new(1019, "Full-screen video"),
                new(1024, "Photo"),
                new(1027, "Cover art"),
                new(1028, "Thumbnail"),
                new(1029, "Thumbnail large"),
                new(1031, "Album art small"),
                new(1032, "Photo list thumbnail"),
            ]),

            ["iPod Photo 4G"] = new("iPod Photo 4G",
            [
                new(1013, "Full-screen photo"),
                new(1015, "Thumbnail"),
                new(1016, "Cover art"),
                new(1019, "Full-screen video"),
            ]),

            ["iPod Touch 1G/2G"] = new("iPod Touch 1G/2G",
            [
                new(3001, "Cover art large"),
                new(3002, "Cover art medium"),
                new(3003, "Cover art small"),
                new(3004, "Photo thumbnail"),
                new(3005, "Cover art square"),
                new(3008, "Full-screen photo"),
                new(3009, "Photo thumbnail"),
                new(3011, "Cover art tiny"),
            ]),

            ["iPod Touch 3G/4G"] = new("iPod Touch 3G/4G",
            [
                new(3001, "Cover art large"),
                new(3002, "Cover art medium"),
                new(3003, "Cover art small"),
                new(3004, "Photo thumbnail"),
                new(3005, "Cover art square"),
                new(3008, "Full-screen photo"),
                new(3009, "Photo thumbnail"),
                new(3011, "Cover art tiny"),
            ]),

            ["iPhone 1G/2G"] = new("iPhone 1G/2G",
            [
                new(3001, "Cover art large"),
                new(3002, "Cover art medium"),
                new(3003, "Cover art small"),
                new(3004, "Photo thumbnail"),
                new(3005, "Cover art square"),
                new(3008, "Full-screen photo"),
                new(3009, "Photo thumbnail"),
                new(3011, "Cover art tiny"),
            ]),

            ["iPhone 3G/3GS"] = new("iPhone 3G/3GS",
            [
                new(3001, "Cover art large"),
                new(3002, "Cover art medium"),
                new(3003, "Cover art small"),
                new(3004, "Photo thumbnail"),
                new(3005, "Cover art square"),
                new(3008, "Full-screen photo"),
                new(3009, "Photo thumbnail"),
                new(3011, "Cover art tiny"),
            ]),

            ["Motorola ROKR E1"] = new("Motorola ROKR E1",
            [
                new(2002, "Cover art"),
                new(2003, "Photo"),
            ]),
        };

        return dict.ToFrozenDictionary();
    }
}
