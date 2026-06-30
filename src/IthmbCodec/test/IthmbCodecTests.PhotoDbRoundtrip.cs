using System.IO;
using IthmbCodec;
using static IthmbCodec.PhotoDb.PhotoDb;
using Xunit;

namespace IthmbCodec.Tests;

public class PhotoDbRoundtripTests
{
    // Format 1017: 56×56 RGB565 (6272 bytes), Format 1031: 42×42 RGB565 (3528 bytes)
    private static List<(int FormatId, byte[] Data)> CreateMockEntries() =>
    [
        (1017, new byte[56 * 56 * 2]),  // 6272 bytes
        (1031, new byte[42 * 42 * 2]),  // 3528 bytes
    ];

    [Fact]
    public void TryBuildPhotoDb_ClassicLayout_EntriesRoundtrip()
    {
        var entries = CreateMockEntries();

        bool built = TryBuildPhotoDb(entries, out var photoDb, mhniHeaderSize: 76, mhniPaddingSize: 64);
        Assert.True(built);
        Assert.NotNull(photoDb);

        bool parsed = TryParsePhotoDb(photoDb, out var parsedEntries, out var frameCount);
        Assert.True(parsed);
        Assert.Equal(2, frameCount);
        Assert.Equal(2, parsedEntries.Count);
        Assert.Equal(1017, parsedEntries[0].FormatId);
        Assert.Equal(1031, parsedEntries[1].FormatId);
    }

    [Fact]
    public void TryBuildPhotoDb_AppleTVLayout_EntriesRoundtrip()
    {
        var entries = CreateMockEntries();

        bool built = TryBuildPhotoDb(entries, out var photoDb, mhniHeaderSize: 84, mhniPaddingSize: 48);
        Assert.True(built);
        Assert.NotNull(photoDb);

        bool parsed = TryParsePhotoDb(photoDb, out var parsedEntries, out var frameCount);
        Assert.True(parsed);
        Assert.Equal(2, frameCount);
        Assert.Equal(2, parsedEntries.Count);
        Assert.Equal(1017, parsedEntries[0].FormatId);
        Assert.Equal(1031, parsedEntries[1].FormatId);
    }

    [Fact]
    public void TryBuildPhotoDb_ZeroPadding_EntriesRoundtrip()
    {
        var entries = CreateMockEntries();

        bool built = TryBuildPhotoDb(entries, out var photoDb, mhniHeaderSize: 76, mhniPaddingSize: 0);
        Assert.True(built);
        Assert.NotNull(photoDb);

        // MHFD(12) + MHSD(16) + 2 × 76 MHNI + pixel data (6272 + 3528)
        int expectedLength = 12 + 16 + 2 * 76 + (56 * 56 * 2) + (42 * 42 * 2);
        Assert.Equal(expectedLength, photoDb.Length);

        bool parsed = TryParsePhotoDb(photoDb, out var parsedEntries, out var frameCount);
        Assert.True(parsed);
        Assert.Equal(2, frameCount);
        Assert.Equal(2, parsedEntries.Count);
        Assert.Equal(1017, parsedEntries[0].FormatId);
        Assert.Equal(1031, parsedEntries[1].FormatId);
    }
}
