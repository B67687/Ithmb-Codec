using System.IO;
using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    /// <summary>Builds a synthetic PhotoDB binary with one 56x56 RGB565 red thumbnail entry.</summary>
    /// <remarks>format_id=1017 matches KnownProfiles (56x56 RGB565).</remarks>
    private static byte[] BuildSyntheticPhotoDb()
    {
        using var ms = new MemoryStream();
        var bw = new BinaryWriter(ms);

        // MHFD header (12 bytes LE)
        bw.Write(0x6466686du); // "mhfd"
        bw.Write(12u);
        bw.Write(1u); // entryCount

        // MHSD section (16 bytes LE)
        bw.Write(0x6473686du); // "mhsd"
        int pixelDataSize = 56 * 56 * 2;
        int sectionSize = 16 + 36 + pixelDataSize; // MHSD(16) + MHNI(36) + pixel data
        bw.Write((uint)sectionSize);
        bw.Write((ushort)0); // index
        bw.Write((ushort)1); // recordType (photos)
        bw.Write(1u); // entryCount

        // MHNI entry (36 bytes LE)
        bw.Write(0x696e686du); // "mhni"
        bw.Write(36u); // headerSize
        bw.Write(1017); // formatId (56x56 RGB565 in KnownProfiles)
        bw.Write(pixelDataSize); // imageSize
        int mhniOffset = 12 + 16; // MHFD(12) + MHSD(16) = 28
        bw.Write(mhniOffset + 36); // ithmbOffset (right after MHNI header)
        bw.Write(56); // width
        bw.Write(56); // height
        bw.Write(0); // hPadding
        bw.Write(0); // vPadding

        // Pixel data: 56*56*2 bytes of RGB565 red (0xF800 LE)
        byte[] pixels = new byte[pixelDataSize];
        for (int i = 0; i < pixelDataSize; i += 2)
        {
            pixels[i] = 0x00;     // Lo byte: 00000 000
            pixels[i + 1] = 0xF8; // Hi byte: 11111 000
        }
        bw.Write(pixels);

        return ms.ToArray();
    }

    // ===================== Parse tests =====================

    [Fact]
    public void PhotoDb_Parse_ValidBinary_ReturnsCorrectEntryCount()
    {
        byte[] photoDb = BuildSyntheticPhotoDb();

        bool parsed = IthmbCodecPlugin.TryParsePhotoDb(photoDb, out var entries, out var frameCount);

        Assert.True(parsed);
        Assert.Equal(1, frameCount);
        Assert.Single(entries);
        Assert.Equal(1017, entries[0].FormatId);
    }

    [Fact]
    public void PhotoDb_Parse_InvalidMagic_ReturnsFalse()
    {
        byte[] data = [0x00, 0x00, 0x00, 0x00];

        bool parsed = IthmbCodecPlugin.TryParsePhotoDb(data, out var entries, out var frameCount);

        Assert.False(parsed);
        Assert.Empty(entries);
        Assert.Equal(0, frameCount);
    }

    [Fact]
    public void PhotoDb_Parse_EmptyBinary_ReturnsFalse()
    {
        byte[] data = [];

        bool parsed = IthmbCodecPlugin.TryParsePhotoDb(data, out var entries, out var frameCount);

        Assert.False(parsed);
        Assert.Empty(entries);
        Assert.Equal(0, frameCount);
    }

    // ===================== Decode integration test =====================

    [Fact]
    public void PhotoDb_EndToEnd_ExtractAndDecode()
    {
        byte[] photoDb = BuildSyntheticPhotoDb();

        bool parsed = IthmbCodecPlugin.TryParsePhotoDb(photoDb, out var entries, out var frameCount);
        Assert.True(parsed);
        Assert.Equal(1, frameCount);
        Assert.Single(entries);

        var (formatId, rawData) = entries[0];
        Assert.Equal(1017, formatId);

        // Look up format in KnownProfiles
        bool foundProfile = IthmbCodecPlugin.KnownProfiles.TryGetValue(formatId, out var profile);
        Assert.True(foundProfile);
        Assert.Equal(56, profile.Width);
        Assert.Equal(56, profile.Height);

        // Decode the raw RGB565 data
        byte* dst = (byte*)NativeMemory.AllocZeroed((nuint)(56 * 4 * 56));
        try
        {
            bool decoded = IthmbCodecPlugin.DecodeRgb565(rawData, dst, 56, 56, littleEndian: true);
            Assert.True(decoded);

            // First pixel should be red (RGB565 0xF800 → BGRA: B=0, G=0, R=255, A=255)
            Assert.Equal(0x00, dst[0]);  // B
            Assert.Equal(0x00, dst[1]);  // G
            Assert.Equal(0xFF, dst[2]);  // R
            Assert.Equal(0xFF, dst[3]);  // A
        }
        finally
        {
            NativeMemory.Free(dst);
        }
    }

    // ===================== Builder tests =====================

    [Fact]
    public void PhotoDb_Build_Roundtrip_MultipleEntries()
    {
        // Build with 2 entries: format 1017 (56x56 RGB565) and 1024 (320x240 RGB565)
        var entry1 = new byte[56 * 56 * 2]; // red pixels
        Array.Fill<byte>(entry1, 0x00); // fill all zeros first, then set odd bytes for RGB565 red
        for (int i = 0; i < entry1.Length; i += 2)
        {
            entry1[i] = 0x00;     // Lo byte: R=0, G=0
            entry1[i + 1] = 0xF8; // Hi byte: R=31, G=0 → RGB565 red (0xF800)
        }

        var entry2 = new byte[320 * 240 * 2]; // checkerboard pattern
        for (int i = 0; i < entry2.Length; i += 4)
        {
            entry2[i] = 0xFF;     // white pixel lo
            entry2[i + 1] = 0xFF; // white pixel hi
            entry2[i + 2] = 0x00; // black pixel lo
            entry2[i + 3] = 0x00; // black pixel hi
        }

        var entries = new List<(int, byte[])> { (1017, entry1), (1024, entry2) };
        bool built = IthmbCodecPlugin.TryBuildPhotoDb(entries, out var photoDb);
        Assert.True(built);
        Assert.NotNull(photoDb);

        // Parse the result and verify roundtrip
        bool parsed = IthmbCodecPlugin.TryParsePhotoDb(photoDb, out var parsedEntries, out var frameCount);
        Assert.True(parsed);
        Assert.Equal(2, frameCount);
        Assert.Equal(1017, parsedEntries[0].FormatId);
        Assert.Equal(1024, parsedEntries[1].FormatId);
        Assert.Equal(entry1.Length, parsedEntries[0].Data.Length);
        Assert.Equal(entry2.Length, parsedEntries[1].Data.Length);
    }

    [Fact]
    public void PhotoDb_Build_EmptyList_ReturnsFalse()
    {
        bool built = IthmbCodecPlugin.TryBuildPhotoDb([], out var output);
        Assert.False(built);
        Assert.Null(output);
    }

    [Fact]
    public void PhotoDb_Build_UnknownFormatId_ReturnsFalse()
    {
        var entries = new List<(int, byte[])> { (9999, [0x00, 0x00]) };
        bool built = IthmbCodecPlugin.TryBuildPhotoDb(entries, out var output);
        Assert.False(built);
        Assert.Null(output);
    }

    // ===================== Integrity check tests =====================

    [Fact]
    public void IntegrityCheck_ValidPhotoDb_ReturnsNoIssues()
    {
        byte[] photoDb = BuildSyntheticPhotoDb();
        var issues = IthmbCodecPlugin.IntegrityCheckPhotoDb(photoDb);
        Assert.Empty(issues);
    }

    [Fact]
    public void IntegrityCheck_EmptyData_ReturnsIssue()
    {
        var issues = IthmbCodecPlugin.IntegrityCheckPhotoDb([]);
        Assert.NotEmpty(issues);
    }

    [Fact]
    public void IntegrityCheck_BadMagic_ReturnsIssue()
    {
        byte[] data = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        var issues = IthmbCodecPlugin.IntegrityCheckPhotoDb(data);
        Assert.NotEmpty(issues);
    }
}
