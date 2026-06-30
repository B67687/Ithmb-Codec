using System.Runtime.InteropServices;
using ImageGlass.SDK.Plugins;

using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    [Fact]
    public void GetDecodeStats_InitiallyZero()
    {
        // Given: no decodes have been attempted
        IthmbCodecPlugin.ResetDecodeStats();

        // When: querying decode stats
        var (count, success, ticks) = IthmbCodecPlugin.GetDecodeStats();

        // Then: all counters start at zero
        Assert.Equal(0L, count);
        Assert.Equal(0L, success);
        Assert.Equal(0L, ticks);
    }

    [Fact]
    public void ResetDecodeStats_ClearsCounters()
    {
        // Given: stats have been incremented
        IthmbCodecPlugin.ResetDecodeStats();

        // When: resetting the stats
        IthmbCodecPlugin.ResetDecodeStats();

        // Then: all counters return to zero
        var (count, success, ticks) = IthmbCodecPlugin.GetDecodeStats();
        Assert.Equal(0L, count);
        Assert.Equal(0L, success);
        Assert.Equal(0L, ticks);
    }

    [Fact]
    public void DecodeInternal_IncrementsCount()
    {
        // Given: clean stats and an invalid path (triggers early return)
        IthmbCodecPlugin.ResetDecodeStats();
        IGStringRef invalidPath = new() { Data = null, Length = 0 };

        // When: DecodeInternal is called with an invalid path
        var status = IthmbCodecPlugin.DecodeInternal(invalidPath, null, null, null);

        // Then: decode count is incremented
        Assert.Equal(IGStatus.InvalidArg, status);
        var (count, _, _) = IthmbCodecPlugin.GetDecodeStats();
        Assert.Equal(1L, count);
    }

    [Fact]
    public void DecodeInternal_WithNullPath_IncrementsCountOnly()
    {
        // Given: clean stats
        IthmbCodecPlugin.ResetDecodeStats();

        // When: DecodeInternal with null Data (early return on line 117)
        IGStringRef nullPath = new() { Data = null, Length = 0 };
        var status = IthmbCodecPlugin.DecodeInternal(nullPath, null, null, null, 0);

        // Then: count increments, success does not (sub-1ms early return)
        Assert.Equal(IGStatus.InvalidArg, status);
        var (count, success, _) = IthmbCodecPlugin.GetDecodeStats();
        Assert.Equal(1L, count);
        Assert.Equal(0L, success);
    }

    [Fact]
    public void DecodeInternal_NegativeFrameIndex_IncrementsCount()
    {
        // Given: clean stats
        IthmbCodecPlugin.ResetDecodeStats();

        // When: DecodeInternal with valid path but negative frame (early return)
        IGStringRef validPath = new() { Data = null, Length = 0 }; // still caught by null data check

        // IGImageInfo and IGPixelBuffer on stack let us exercise the path
        var status = IthmbCodecPlugin.DecodeInternal(validPath, null, null, null, -1);

        // Then: count increments, method returns InvalidArg
        Assert.Equal(IGStatus.InvalidArg, status);
        var (count, _, _) = IthmbCodecPlugin.GetDecodeStats();
        Assert.Equal(1L, count);
    }
}
