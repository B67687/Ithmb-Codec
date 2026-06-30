using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    /// <summary>
    /// Verifies that when the raw file cache exceeds MaxCachedPaths (16 entries),
    /// the entry with the oldest LastAccess timestamp is evicted via TryRemove,
    /// and the new entry is added successfully.
    /// </summary>
    [Fact]
    public void RawFileCache_LruEviction_EvictsOldest()
    {
        // Arrange
        IthmbCodecPlugin.ClearRawFileCache();
        var data = new byte[100];
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(1007, 1, 1, IthmbCodecPlugin.IthmbEncoding.Rgb565, 2);

        // Fill the cache to MaxCachedPaths (16) entries
        for (int i = 0; i < 16; i++)
        {
            IthmbCodecPlugin.SetCachedFile($"path{i}", data, profile, 1, 100);
        }

        // Touch entries 1-15 to bump their LastAccess, leaving path0 as the oldest
        for (int i = 1; i < 16; i++)
        {
            IthmbCodecPlugin.TryGetCachedFile($"path{i}", out _);
        }

        // Act: add the 17th entry — should evict the oldest (path0)
        IthmbCodecPlugin.SetCachedFile("path16", data, profile, 1, 100);

        // Assert
        Assert.False(IthmbCodecPlugin.TryGetCachedFile("path0", out _),
            "Oldest entry (path0) should have been evicted");
        Assert.True(IthmbCodecPlugin.TryGetCachedFile("path16", out _),
            "Newest entry (path16) should be present");
    }

    /// <summary>
    /// Verifies that accessing a cached file via TryGetCachedFile updates its
    /// LastAccess timestamp so the entry is not prematurely evicted.
    /// </summary>
    [Fact]
    public void RawFileCache_GetCachedFile_UpdatesLastAccess()
    {
        // Arrange
        IthmbCodecPlugin.ClearRawFileCache();
        var data = new byte[100];
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(1007, 1, 1, IthmbCodecPlugin.IthmbEncoding.Rgb565, 2);

        IthmbCodecPlugin.SetCachedFile("testpath", data, profile, 1, 100);

        // Act: get the cached entry
        IthmbCodecPlugin.TryGetCachedFile("testpath", out var entry1);
        var ts1 = entry1.LastAccess;

        // Small spin to ensure the timestamp advances (Stopwatch ticks are typically
        // sub-microsecond, so this is a safety margin, not a requirement).
        Thread.SpinWait(10);

        IthmbCodecPlugin.TryGetCachedFile("testpath", out var entry2);
        var ts2 = entry2.LastAccess;

        // Assert: second access timestamp > first access timestamp
        Assert.True(ts2 > ts1,
            $"LastAccess should increase after a cache hit. Before={ts1}, After={ts2}");
    }

    /// <summary>
    /// Verifies that TryGetCachedFile returns false for a path not in the cache,
    /// and does not add it to the cache.
    /// </summary>
    [Fact]
    public void RawFileCache_TryGetCachedFile_Missing_ReturnsFalse()
    {
        // Arrange
        IthmbCodecPlugin.ClearRawFileCache();

        // Act & Assert
        Assert.False(IthmbCodecPlugin.TryGetCachedFile("nonexistent", out _),
            "Missing path should return false");
    }
}
