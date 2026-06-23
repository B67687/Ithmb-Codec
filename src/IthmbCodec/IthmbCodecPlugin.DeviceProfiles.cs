// Per-generation iPod format ID tables synthesized from iOpenPod, OrgZ, libgpod, gnupod,
// and 22-repo research sweep. Maps each known iPod/iPhone generation to the format IDs
// it generates for its thumbnail caches (PhotoDB & ArtworkDB). Only format IDs present
// in KnownProfiles are included; unknown IDs (e.g. iPod Photo 4G format 1025) are omitted.

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
                new(1019, "720×480 YUV422 interlaced full-screen"),
                new(1024, "320×240 RGB565 photo"),
                new(1027, "100×100 RGB565 cover art"),
                new(1028, "100×100 RGB565"),
                new(1029, "200×200 RGB565"),
                new(1031, "42×42 RGB565"),
                new(1032, "42×37 RGB565"),
            ]),

            ["iPod Classic 5.5G (Enhanced)"] = new("iPod Classic 5.5G (Enhanced)",
            [
                new(1019, "720×480 YUV422 interlaced full-screen"),
                new(1024, "320×240 RGB565 photo"),
                new(1027, "100×100 RGB565 cover art"),
                new(1028, "100×100 RGB565"),
                new(1029, "200×200 RGB565"),
                new(1031, "42×42 RGB565"),
                new(1032, "42×37 RGB565"),
                new(1055, "128×128 RGB565 cover art medium"),
                new(1056, "80×80 RGB565"),
            ]),

            ["iPod Classic 6G (Thin)"] = new("iPod Classic 6G (Thin)",
            [
                new(1024, "320×240 RGB565 photo"),
                new(1055, "128×128 RGB565 cover art"),
                new(1060, "320×320 RGB565 cover art large"),
                new(1061, "56×56 RGB565 cover art small"),
                new(1066, "64×64 RGB565 photo"),
                new(1067, "720×480 YCbCr420 padded full-screen"),
                new(1068, "128×128 RGB565 cover art"),
            ]),

            ["iPod Nano 1G"] = new("iPod Nano 1G",
            [
                new(1024, "320×240 RGB565"),
                new(1027, "100×100 RGB565"),
            ]),

            ["iPod Nano 2G"] = new("iPod Nano 2G",
            [
                new(1019, "720×480 YUV422"),
                new(1027, "100×100"),
                new(1028, "100×100"),
                new(1029, "200×200"),
                new(1032, "42×37"),
            ]),

            ["iPod Nano 3G"] = new("iPod Nano 3G",
            [
                new(1066, "64×64 RGB565"),
                new(1067, "720×480 YCbCr420 padded"),
                new(1068, "128×128"),
                new(1071, "240×240"),
                new(1073, "240×240"),
                new(1074, "50×50"),
            ]),

            ["iPod Nano 4G"] = new("iPod Nano 4G",
            [
                new(1071, "240×240"),
                new(1073, "240×240"),
                new(1074, "50×50"),
                new(1078, "80×80"),
                new(1079, "80×80"),
                new(1083, "240×320"),
                new(1084, "240×240"),
                new(1085, "88×88"),
                new(1087, "384×384"),
                new(1089, "58×58"),
                new(1092, "80×80"),
                new(1093, "512×512"),
            ]),

            ["iPod Nano 5G"] = new("iPod Nano 5G",
            [
                new(1087, "384×384 RGB565"),
                new(1092, "80×80"),
                new(1093, "512×512"),
            ]),

            ["iPod Nano 6G"] = new("iPod Nano 6G",
            [
                new(1084, "240×240"),
                new(1092, "80×80"),
                new(1093, "512×512"),
            ]),

            ["iPod Nano 7G"] = new("iPod Nano 7G",
            [
                new(1007, "480×864 RGB565 full-res"),
                new(1010, "240×240 RGB565 cover art"),
            ]),

            ["iPod Video 5G"] = new("iPod Video 5G",
            [
                new(1019, "720×480 YUV422"),
                new(1024, "320×240"),
                new(1027, "100×100"),
                new(1028, "100×100"),
                new(1029, "200×200"),
                new(1031, "42×42"),
                new(1032, "42×37"),
            ]),

            ["iPod Mini 1G/2G"] = new("iPod Mini 1G/2G",
            [
                new(1024, "320×240 RGB565"),
                new(1027, "100×100"),
            ]),

            ["iPod Photo 4G"] = new("iPod Photo 4G",
            [
                new(1013, "220×176 RGB565 big-endian"),
                new(1015, "130×88 RGB565"),
                new(1016, "140×140"),
                new(1019, "720×480 YUV422"),
                // 1025 (320×240) skipped — not present in KnownProfiles
            ]),

            ["iPod Touch 1G/2G"] = new("iPod Touch 1G/2G",
            [
                new(3001, "256×256 RGB555"),
                new(3002, "128×128 RGB555"),
                new(3003, "64×64 RGB555"),
                new(3004, "56×55 RGB555"),
                new(3005, "320×320 RGB555"),
                new(3008, "640×480 RGB555"),
                new(3009, "160×120 RGB555"),
                new(3011, "80×79 RGB555"),
            ]),

            ["iPod Touch 3G/4G"] = new("iPod Touch 3G/4G",
            [
                new(3001, "256×256 RGB555"),
                new(3002, "128×128 RGB555"),
                new(3003, "64×64 RGB555"),
                new(3004, "56×55 RGB555"),
                new(3005, "320×320 RGB555"),
                new(3008, "640×480 RGB555"),
                new(3009, "160×120 RGB555"),
                new(3011, "80×79 RGB555"),
            ]),

            ["iPhone 1G/2G"] = new("iPhone 1G/2G",
            [
                new(3001, "256×256 RGB555"),
                new(3002, "128×128 RGB555"),
                new(3003, "64×64 RGB555"),
                new(3004, "56×55 RGB555"),
                new(3005, "320×320 RGB555"),
                new(3008, "640×480 RGB555"),
                new(3009, "160×120 RGB555"),
                new(3011, "80×79 RGB555"),
            ]),

            ["iPhone 3G/3GS"] = new("iPhone 3G/3GS",
            [
                new(3001, "256×256 RGB555"),
                new(3002, "128×128 RGB555"),
                new(3003, "64×64 RGB555"),
                new(3004, "56×55 RGB555"),
                new(3005, "320×320 RGB555"),
                new(3008, "640×480 RGB555"),
                new(3009, "160×120 RGB555"),
                new(3011, "80×79 RGB555"),
            ]),

            ["Motorola ROKR E1"] = new("Motorola ROKR E1",
            [
                new(2002, "50×50 RGB565 big-endian"),
                new(2003, "150×150 RGB565 big-endian"),
            ]),
        };

        return dict.ToFrozenDictionary();
    }
}
