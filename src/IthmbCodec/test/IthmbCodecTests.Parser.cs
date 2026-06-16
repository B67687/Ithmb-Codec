using System;
using System.Collections.Generic;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe class ParserTests
{
    [Fact]
    public void ParseProfilesJson_WithComments_ParsesCorrectly()
    {
        string json = "[\n  // This is a comment\n  {\n    \"prefix\": 1013,\n    \"width\": 220,\n    \"height\": 176,\n    \"encoding\": \"rgb565\",\n    \"frameBytes\": 77440\n  }\n]";
        var output = new Dictionary<int, IthmbCodecPlugin.IthmbVariantProfile>();
        IthmbCodecPlugin.ParseProfilesJson(json, output);

        Assert.Single(output);
        Assert.True(output.ContainsKey(1013));
        Assert.Equal(220, output[1013].Width);
        Assert.Equal(176, output[1013].Height);
        Assert.Equal(77440, output[1013].FrameByteLength);
    }

    [Fact]
    public void ParseProfilesJson_EmptyArray_NoEntries()
    {
        var output = new Dictionary<int, IthmbCodecPlugin.IthmbVariantProfile>();
        IthmbCodecPlugin.ParseProfilesJson("[]", output);
        Assert.Empty(output);
    }

    [Fact]
    public void ParseProfilesJson_Malformed_SilentlyFails()
    {
        var output = new Dictionary<int, IthmbCodecPlugin.IthmbVariantProfile>();
        IthmbCodecPlugin.ParseProfilesJson("{bad json}", output);
        Assert.Empty(output);
    }

    [Fact]
    public void ParseProfilesJson_WithCropFields_ParsesCorrectly()
    {
        string json = "[\n  {\n    \"prefix\": 1007,\n    \"width\": 480,\n    \"height\": 864,\n    \"encoding\": \"rgb565\",\n    \"frameBytes\": 829440,\n    \"cropX\": 20,\n    \"cropY\": 30,\n    \"cropWidth\": 440,\n    \"cropHeight\": 804\n  }\n]";
        var output = new Dictionary<int, IthmbCodecPlugin.IthmbVariantProfile>();
        IthmbCodecPlugin.ParseProfilesJson(json, output);

        Assert.Single(output);
        Assert.True(output.ContainsKey(1007));
        Assert.Equal(20, output[1007].CropX);
        Assert.Equal(30, output[1007].CropY);
        Assert.Equal(440, output[1007].CropWidth);
        Assert.Equal(804, output[1007].CropHeight);
    }
}
