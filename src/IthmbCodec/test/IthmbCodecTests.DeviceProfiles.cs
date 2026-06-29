using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    [Fact]
    public void DeviceProfiles_HasAtLeastTwelveEntries()
    {
        Assert.True(IthmbCodecPlugin.DeviceProfiles.Count >= 12);
        foreach (var (name, profile) in IthmbCodecPlugin.DeviceProfiles)
        {
            Assert.False(string.IsNullOrWhiteSpace(name));
            Assert.NotNull(profile.Formats);
            Assert.True(profile.Formats.Length > 0, $"Device {name} has no formats");
        }
    }

    [Fact]
    public void DeviceProfiles_AllFormatIdsExistInKnownProfiles()
    {
        foreach (var (name, profile) in IthmbCodecPlugin.DeviceProfiles)
        {
            foreach (var fmt in profile.Formats)
            {
                Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(fmt.FormatId),
                    $"Device {name} references unknown format {fmt.FormatId}");
            }
        }
    }

    [Fact]
    public void DeviceProfiles_Classic6G_HasAllExpectedFormats()
    {
        var classic6G = IthmbCodecPlugin.DeviceProfiles["iPod Classic 6G (Thin)"];
        var formatIds = classic6G.Formats.Select(f => f.FormatId).ToHashSet();
        int[] expected = [1024, 1055, 1060, 1061, 1066, 1067, 1068];
        foreach (var id in expected)
            Assert.Contains(id, formatIds);
    }

    [Fact]
    public void DeviceProfiles_Nano4G_HasAllExpectedFormats()
    {
        var nano4G = IthmbCodecPlugin.DeviceProfiles["iPod Nano 4G"];
        var formatIds = nano4G.Formats.Select(f => f.FormatId).ToHashSet();
        int[] expected = [1024, 1055, 1066, 1068, 1071, 1074, 1078, 1079, 1083, 1084];
        foreach (var id in expected)
            Assert.Contains(id, formatIds);
    }

    [Fact]
    public void DeviceProfiles_Nano5G_HasExpectedFormats()
    {
        var nano5G = IthmbCodecPlugin.DeviceProfiles["iPod Nano 5G"];
        var formatIds = nano5G.Formats.Select(f => f.FormatId).ToHashSet();
        int[] expected = [1056, 1066, 1073, 1074, 1078, 1079, 1087];
        foreach (var id in expected)
            Assert.Contains(id, formatIds);
    }

    [Fact]
    public void DeviceProfiles_Nano6G_HasExpectedFormats()
    {
        var nano6G = IthmbCodecPlugin.DeviceProfiles["iPod Nano 6G"];
        var formatIds = nano6G.Formats.Select(f => f.FormatId).ToHashSet();
        int[] expected = [1073, 1074, 1085, 1089, 1092, 1093];
        foreach (var id in expected)
            Assert.Contains(id, formatIds);
    }

    [Fact]
    public void DeviceProfiles_Nano7G_HasExpectedFormats()
    {
        var nano7G = IthmbCodecPlugin.DeviceProfiles["iPod Nano 7G"];
        var formatIds = nano7G.Formats.Select(f => f.FormatId).ToHashSet();
        int[] expected = [1007, 1010, 1013, 1015, 1016];
        foreach (var id in expected)
            Assert.Contains(id, formatIds);
    }

    [Fact]
    public void TryResolveProfile_1013_Nano7GDataSize_Returns50x50()
    {
        // Nano 7G 1013 = 50×50 RGB565 = 5000 bytes
        var data = new byte[50 * 50 * 2];
        Assert.True(IthmbCodecPlugin.TryResolveProfile(1013, data.AsSpan(), out var profile));
        Assert.Equal(50, profile.Width);
        Assert.Equal(50, profile.Height);
        Assert.Equal(IthmbCodecPlugin.IthmbEncoding.Rgb565, profile.Encoding);
        Assert.True(profile.LittleEndian);
        Assert.Equal(0, profile.Rotation);
    }

    [Fact]
    public void TryResolveProfile_1013_GlobalDataSize_Returns220x176()
    {
        // Global 1013 = 220×176 RGB565 = 77440 bytes
        var data = new byte[220 * 176 * 2];
        Assert.True(IthmbCodecPlugin.TryResolveProfile(1013, data.AsSpan(), out var profile));
        Assert.Equal(220, profile.Width);
        Assert.Equal(176, profile.Height);
        Assert.Equal(IthmbCodecPlugin.IthmbEncoding.Rgb565, profile.Encoding);
        Assert.False(profile.LittleEndian);
        Assert.Equal(90, profile.Rotation);
    }

    [Fact]
    public void TryResolveProfile_1015_Nano7GDataSize_Returns58x58()
    {
        // Nano 7G 1015 = 58×58 RGB565 = 6728 bytes
        var data = new byte[58 * 58 * 2];
        Assert.True(IthmbCodecPlugin.TryResolveProfile(1015, data.AsSpan(), out var profile));
        Assert.Equal(58, profile.Width);
        Assert.Equal(58, profile.Height);
    }

    [Fact]
    public void TryResolveProfile_1015_GlobalDataSize_Returns130x88()
    {
        // Global 1015 = 130×88 RGB565 = 22880 bytes
        var data = new byte[130 * 88 * 2];
        Assert.True(IthmbCodecPlugin.TryResolveProfile(1015, data.AsSpan(), out var profile));
        Assert.Equal(130, profile.Width);
        Assert.Equal(88, profile.Height);
    }

    [Fact]
    public void TryResolveProfile_UnknownFormatId_ReturnsFalse()
    {
        var data = new byte[100];
        Assert.False(IthmbCodecPlugin.TryResolveProfile(9999, data.AsSpan(), out _));
    }

    [Fact]
    public void TryResolveProfile_FormatWithoutAlternates_ReturnsGlobalProfile()
    {
        // 1024 has no alternates, should always return global
        var data = new byte[320 * 240 * 2];
        Assert.True(IthmbCodecPlugin.TryResolveProfile(1024, data.AsSpan(), out var profile));
        Assert.Equal(320, profile.Width);
        Assert.Equal(240, profile.Height);
    }

    [Fact]
    public void DeviceProfiles_TouchFormatsAreAllRgb555()
    {
        var touch1G = IthmbCodecPlugin.DeviceProfiles["iPod Touch 1G/2G"];
        foreach (var fmt in touch1G.Formats)
        {
            var profile = IthmbCodecPlugin.KnownProfiles[fmt.FormatId];
            Assert.True(profile.Encoding == IthmbCodecPlugin.IthmbEncoding.Rgb555
                || profile.Encoding == IthmbCodecPlugin.IthmbEncoding.ReorderedRgb555,
                $"Touch format {fmt.FormatId} has unexpected encoding {profile.Encoding}");
        }
    }

    [Fact]
    public void KnownProfiles_1042_1043_HaveCorrectDimensions()
    {
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(1042));
        Assert.Equal(320, IthmbCodecPlugin.KnownProfiles[1042].Width);
        Assert.Equal(240, IthmbCodecPlugin.KnownProfiles[1042].Height);
        Assert.Equal(IthmbCodecPlugin.IthmbEncoding.Rgb565, IthmbCodecPlugin.KnownProfiles[1042].Encoding);

        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(1043));
        Assert.Equal(130, IthmbCodecPlugin.KnownProfiles[1043].Width);
        Assert.Equal(88, IthmbCodecPlugin.KnownProfiles[1043].Height);
        Assert.Equal(IthmbCodecPlugin.IthmbEncoding.Rgb565, IthmbCodecPlugin.KnownProfiles[1043].Encoding);
    }

    [Fact]
    public void KnownProfiles_3006_3007_HaveSlotPadding()
    {
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(3006));
        Assert.Equal(56, IthmbCodecPlugin.KnownProfiles[3006].Width);
        Assert.Equal(56, IthmbCodecPlugin.KnownProfiles[3006].Height);
        Assert.True(IthmbCodecPlugin.KnownProfiles[3006].IsPadded);
        Assert.Equal(8192, IthmbCodecPlugin.KnownProfiles[3006].SlotSize);

        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(3007));
        Assert.Equal(88, IthmbCodecPlugin.KnownProfiles[3007].Width);
        Assert.Equal(88, IthmbCodecPlugin.KnownProfiles[3007].Height);
        Assert.True(IthmbCodecPlugin.KnownProfiles[3007].IsPadded);
        Assert.Equal(16384, IthmbCodecPlugin.KnownProfiles[3007].SlotSize);
    }

    [Fact]
    public void DeviceProfiles_NoDuplicateFormatsWithinDevice()
    {
        foreach (var (name, profile) in IthmbCodecPlugin.DeviceProfiles)
        {
            var ids = profile.Formats.Select(f => f.FormatId).ToArray();
            Assert.Equal(ids.Length, ids.Distinct().Count());
        }
    }

    [Fact]
    public void KnownProfiles_JsonBaseline_CountAndKeyEntries()
    {
        // Verify the embedded JSON parses to the expected number of profiles
        Assert.Equal(53, IthmbCodecPlugin.KnownProfiles.Count);

        // Spot-check critical entries that are easy to get wrong
        var p1007 = IthmbCodecPlugin.KnownProfiles[1007];
        Assert.Equal(480, p1007.Width);
        Assert.Equal(864, p1007.Height);
        Assert.Equal(IthmbCodecPlugin.IthmbEncoding.Rgb565, p1007.Encoding);
        Assert.Equal(829440, p1007.FrameByteLength);

        var p1013 = IthmbCodecPlugin.KnownProfiles[1013];
        Assert.Equal(220, p1013.Width);
        Assert.Equal(176, p1013.Height);
        Assert.Equal(90, p1013.Rotation);
        Assert.False(p1013.LittleEndian);

        var p1067 = IthmbCodecPlugin.KnownProfiles[1067];
        Assert.Equal(IthmbCodecPlugin.IthmbEncoding.Ycbcr420, p1067.Encoding);
        Assert.True(p1067.IsPadded);

        var p3006 = IthmbCodecPlugin.KnownProfiles[3006];
        Assert.Equal(IthmbCodecPlugin.IthmbEncoding.Rgb555, p3006.Encoding);
        Assert.Equal(8192, p3006.SlotSize);
        Assert.True(p3006.IsPadded);

        var p2002 = IthmbCodecPlugin.KnownProfiles[2002];
        Assert.False(p2002.LittleEndian);

        var p1061 = IthmbCodecPlugin.KnownProfiles[1061];
        Assert.True(p1061.UseMhniDimensions);

        var p1062 = IthmbCodecPlugin.KnownProfiles[1062];
        Assert.Equal(56, p1062.Width);
        Assert.Equal(56, p1062.Height);
        Assert.Equal(IthmbCodecPlugin.IthmbEncoding.Rgb565, p1062.Encoding);
        Assert.Equal(6272, p1062.FrameByteLength);
    }
}
