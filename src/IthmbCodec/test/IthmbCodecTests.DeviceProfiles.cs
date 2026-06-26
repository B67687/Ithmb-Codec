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
        int[] expected = [1071, 1073, 1074, 1078, 1079, 1083, 1084, 1085, 1087, 1089, 1092, 1093];
        foreach (var id in expected)
            Assert.Contains(id, formatIds);
    }

    [Fact]
    public void DeviceProfiles_TouchFormatsAreAllRgb555()
    {
        var touch1G = IthmbCodecPlugin.DeviceProfiles["iPod Touch 1G/2G"];
        foreach (var fmt in touch1G.Formats)
        {
            var profile = IthmbCodecPlugin.KnownProfiles[fmt.FormatId];
            Assert.Equal(IthmbCodecPlugin.IthmbEncoding.Rgb555, profile.Encoding);
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
}
