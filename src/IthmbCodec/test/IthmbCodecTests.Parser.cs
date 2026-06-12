using System;
using System.Collections.Generic;
using System.Reflection;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe class ParserTests
{
    [Fact]
    public void ParseProfilesJson_WithComments_ParsesCorrectly()
    {
        string json = "[\n  // This is a comment\n  {\n    \"prefix\": 1013,\n    \"width\": 220,\n    \"height\": 176,\n    \"encoding\": \"rgb565\",\n    \"frameBytes\": 77440\n  }\n]";
        var output = new Dictionary<int, IthmbCodecPlugin.IthmbVariantProfile>();
        var method = typeof(IthmbCodecPlugin).GetMethod("ParseProfilesJson",
            BindingFlags.NonPublic | BindingFlags.Static);
        Assert.NotNull(method);
        method.Invoke(null, [json, output]);

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
        var method = typeof(IthmbCodecPlugin).GetMethod("ParseProfilesJson",
            BindingFlags.NonPublic | BindingFlags.Static);
        Assert.NotNull(method);
        method.Invoke(null, ["[]", output]);
        Assert.Empty(output);
    }

    [Fact]
    public void ParseProfilesJson_Malformed_SilentlyFails()
    {
        var output = new Dictionary<int, IthmbCodecPlugin.IthmbVariantProfile>();
        var method = typeof(IthmbCodecPlugin).GetMethod("ParseProfilesJson",
            BindingFlags.NonPublic | BindingFlags.Static);
        Assert.NotNull(method);
        method.Invoke(null, ["{bad json}", output]);
        Assert.Empty(output);
    }
}
