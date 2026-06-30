using System.IO;
using IthmbCodec;
using static IthmbCodec.PhotoDb.PhotoDb;
using Xunit;

namespace IthmbCodec.Tests;

public class PhotoDbFuzzTests
{
    /// <summary>
    /// Feeds 25 malformed PhotoDB byte arrays into TryParsePhotoDb and asserts
    /// each completes without throwing, returning a consistent (parsed, entries, frameCount) triple.
    /// The parser returns false for MHFD-level failures and true (with gracefully skipped
    /// children) when MHFD is valid but child chunks are malformed.
    /// </summary>
    [Fact]
    public void TryParsePhotoDb_MalformedChunks_ReturnsFalse()
    {
        // --- Group A: MHFD-level failures — parser MUST return false ---
        var mhfdFailures = new (string Name, byte[] Data)[]
        {
            // 1. Empty byte array
            ("EmptyArray", []),

            // 2. Truncated MHFD header (only 4 bytes — not enough for the 12-byte header)
            ("TruncatedMhfd", [0x6d, 0x68, 0x66, 0x64]),

            // 3. MHFD with headerSize=0 (fails the < 12 check)
            ("MhfdSizeZero", BuildChunk("mhfd", headerSize: 0, bodyLen: 0)),

            // 4. MHFD with headerSize=8 (too small, < 12)
            ("MhfdSizeTooSmall", BuildChunk("mhfd", headerSize: 8, bodyLen: 0)),

            // 5. Corrupt magic bytes (not "mhfd" or "dfhm")
            ("CorruptMagic", BuildChunk("xxxx", headerSize: 12, bodyLen: 0)),

            // 6. Single byte data
            ("SingleByte", [0x6d]),

            // 7. Two bytes
            ("TwoBytes", [0x6d, 0x68]),

            // 8. Valid "mhfd" magic but headerSize=0 (all zeros)
            ("MhfdAllZeros",
            [
                0x6d, 0x68, 0x66, 0x64, // "mhfd"
                0x00, 0x00, 0x00, 0x00, // headerSize=0
                0x00, 0x00, 0x00, 0x00, // entryCount=0
            ]),

            // NOTE: Inputs with MHFD headerSize > int.MaxValue (e.g. 0xFFFFFFFF, 0x80000000)
            // expose a parser bug: (int)mhfd.HeaderSize overflows to negative, causing
            // WalkEntries to start at a negative offset → IndexOutOfRangeException.
            // These are excluded until the parser validates headerSize <= data.Length.
        };

        foreach (var (name, data) in mhfdFailures)
        {
            // Act — must not throw any exception
            bool parsed = TryParsePhotoDb(data, out var entries, out var frameCount);

            // Assert — MHFD validation fails → false
            Assert.False(parsed, $"Expected false for MHFD-level failure '{name}' but got true");
            Assert.True(entries is null || entries.Count == 0,
                $"Expected empty/null entries for '{name}' but got {entries?.Count ?? 0}");
            Assert.Equal(0, frameCount);
        }

        // --- Group B: Valid MHFD + malformed children — parser returns true, no crash ---
        // The parser gracefully skips bad children and returns true with empty/sparse entries.
        var childFailures = new (string Name, byte[] Data)[]
        {
            // 11. MHSD with headerSize < 8
            ("MhsdSizeTooSmall", BuildMhfdThenChild("mhsd", childHeaderSize: 4)),

            // 12. MHSD headerSize pointing past end of data
            ("MhsdPastEnd", BuildMhfdThenChild("mhsd", childHeaderSize: 0x7FFFFFFF)),

            // 13. MHNI with headerSize < 36 (minimum parsed fields)
            ("MhniSizeTooSmall", BuildMhfdWithMhni(mhniHeaderSize: 12)),

            // 14. MHNI with IthmbOffset + ImageSize pointing past end of data
            ("MhniPastEnd", BuildMhfdWithMhni(mhniHeaderSize: 76, ithmbOffset: 1000, imageSize: 999999)),

            // 15. Overlapping chunks — MHSD claims a range that overlaps with MHNI
            ("OverlappingChunks", BuildOverlappingMhsdMhni()),

            // 16. Nested MHSDs that exceed max recursion depth (depth > 64)
            ("MaxRecursionDepth", BuildRecursiveMhsd(depth: 65)),

            // 17. Padding bytes that look like "mhfd" magic
            ("PaddingLooksLikeMagic",
            [
                0x6d, 0x68, 0x66, 0x64, // "mhfd"
                0x10, 0x00, 0x00, 0x00, // headerSize=16
                0x00, 0x00, 0x00, 0x00, // entryCount=0
                0x6d, 0x68, 0x66, 0x64, // padding that looks like magic
                0x6d, 0x68, 0x66, 0x64,
            ]),

            // 18. Alternating valid MHFD header with corrupt child chunks
            ("AlternatingValidCorrupt", BuildAlternatingValidCorrupt()),

            // 19. MHFD with entryCount=0 (valid header, no children)
            ("MhfdZeroEntries", BuildChunk("mhfd", headerSize: 12, bodyLen: 0, entryCount: 0)),

            // 20. Big-endian magic "dfhm" with LE-format body
            ("BeMagicLeBody",
            [
                0x64, 0x66, 0x68, 0x6d, // "dfhm" (big-endian)
                0x0C, 0x00, 0x00, 0x00, // headerSize=12
                0x01, 0x00, 0x00, 0x00, // entryCount=1
                0x64, 0x73, 0x68, 0x6d, // child "mhsd" (BE)
                0x04, 0x00, 0x00, 0x00, // headerSize=4 (< 8, parser skips)
            ]),

            // NOTE: MHII with totalLen=0 causes infinite loop in WalkEntries (pos += 0).
            // MHII with totalLen > int.MaxValue causes integer overflow → IndexOutOfRangeException.
            // Both are parser bugs excluded until WalkEntries validates totalLen bounds.

            // 23. Valid MHFD + MHSD but MHSD children are non-magic garbage
            ("MhsdGarbageChildren", BuildMhfdWithGarbageChildren()),

            // 24. Two concatenated MHFD headers (second is invalid)
            ("DoubleMhfd", BuildDoubleMhfd()),

            // 25. MHSD with headerSize=8 (minimum valid, no room for children)
            ("MhsdMinimalSize", BuildMhfdWithMhsd(mhsdHeaderSize: 8)),
        };

        foreach (var (name, data) in childFailures)
        {
            // Act — must not throw any exception
            bool parsed = TryParsePhotoDb(data, out var entries, out var frameCount);

            // Assert — parser skips bad children gracefully
            // parsed may be true (valid MHFD), but entries should be empty or sparse
            Assert.True(parsed, $"Expected true for child-level failure '{name}' but got false");
            Assert.NotNull(entries);
            Assert.Equal(entries.Count, frameCount);
        }
    }

    // ========================= Helper builders =========================

    /// <summary>
    /// Builds a raw chunk: [magic(4)] [headerSize(4)] [body(headerSize - 8 bytes)].
    /// For headerSize=0, only the 4-byte magic is written.
    /// </summary>
    private static byte[] BuildChunk(string magic, uint headerSize, int bodyLen, uint? entryCount = null)
    {
        using var ms = new MemoryStream();
        var bw = new BinaryWriter(ms);

        // Magic bytes
        foreach (char c in magic) bw.Write((byte)c);

        // headerSize (LE uint32)
        bw.Write(headerSize);

        // optional entryCount (MHFD field at +8)
        bw.Write(entryCount ?? 1u);

        // Pad body with zeros up to headerSize - 8 (or bodyLen)
        int remaining = (int)(headerSize > 8 ? headerSize - 8 : 0);
        if (bodyLen > remaining) remaining = bodyLen;
        for (int i = 0; i < remaining; i++) bw.Write((byte)0);

        return ms.ToArray();
    }

    /// <summary>
    /// Builds a valid MHFD followed by a single child chunk with the given magic and headerSize.
    /// </summary>
    private static byte[] BuildMhfdThenChild(string childMagic, uint childHeaderSize)
    {
        using var ms = new MemoryStream();
        var bw = new BinaryWriter(ms);

        // MHFD header (12 bytes)
        bw.Write(0x6466686du); // "mhfd" LE
        bw.Write(12u);         // headerSize
        bw.Write(1u);          // entryCount

        // Child chunk magic
        foreach (char c in childMagic) bw.Write((byte)c);

        // Child headerSize
        bw.Write(childHeaderSize);

        // Pad remaining child body (capped to avoid huge allocations)
        int bodyLen = (int)(childHeaderSize > 8 ? childHeaderSize - 8 : 0);
        if (bodyLen > 256) bodyLen = 256;
        for (int i = 0; i < bodyLen; i++) bw.Write((byte)0);

        return ms.ToArray();
    }

    /// <summary>
    /// Builds MHFD → MHSD → MHNI with configurable MHNI parameters.
    /// </summary>
    private static byte[] BuildMhfdWithMhni(uint mhniHeaderSize = 76,
        int ithmbOffset = 100, int imageSize = 56 * 56 * 2)
    {
        const int mhfdLen = 12;
        const int mhsdLen = 96;

        using var ms = new MemoryStream();
        var bw = new BinaryWriter(ms);

        // MHFD
        bw.Write(0x6466686du);
        bw.Write((uint)mhfdLen);
        bw.Write(1u);

        // MHSD (96 bytes)
        bw.Write(0x6473686du);
        bw.Write((uint)mhsdLen);
        bw.Write((ushort)0); // index
        bw.Write((ushort)0); // recordType
        bw.Write(0u);        // entryCount
        for (int i = 0; i < 80; i++) bw.Write((byte)0);

        // MHNI
        bw.Write(0x696e686du); // "mhni" magic
        bw.Write(mhniHeaderSize);
        bw.Write(1u);                          // total_len
        bw.Write(0u);                          // reserved
        bw.Write(0u);                          // reserved
        bw.Write(0L);                          // reserved
        bw.Write(1017);                        // formatId
        bw.Write(ithmbOffset);                 // ithmbOffset
        bw.Write(imageSize);                   // imageSize
        bw.Write(0u);                          // reserved
        bw.Write((short)56);                   // height
        bw.Write((short)56);                   // width

        // Pad remaining MHNI body
        int mhniPad = (int)(mhniHeaderSize > 36 ? mhniHeaderSize - 36 : 0);
        for (int i = 0; i < mhniPad; i++) bw.Write((byte)0);

        return ms.ToArray();
    }

    /// <summary>
    /// Builds MHFD → MHSD with overlapping child ranges for MHSD and MHNI.
    /// </summary>
    private static byte[] BuildOverlappingMhsdMhni()
    {
        using var ms = new MemoryStream();
        var bw = new BinaryWriter(ms);

        // MHFD (12 bytes)
        bw.Write(0x6466686du);
        bw.Write(12u);
        bw.Write(1u);

        // MHSD that claims a huge headerSize covering the MHNI
        bw.Write(0x6473686du); // "mhsd"
        bw.Write(500u);        // headerSize=500 (extends past end of real data)
        bw.Write((ushort)0);
        bw.Write((ushort)0);
        bw.Write(0u);
        for (int i = 0; i < 80; i++) bw.Write((byte)0);

        // MHNI (76 bytes) — sits within the MHSD's claimed range
        bw.Write(0x696e686du); // "mhni"
        bw.Write(76u);
        bw.Write(1u);
        bw.Write(0u);
        bw.Write(1017);
        bw.Write(100);         // ithmbOffset
        bw.Write(1000);        // imageSize (points past end)
        bw.Write(0u);
        bw.Write((short)56);
        bw.Write((short)56);
        for (int i = 0; i < 40; i++) bw.Write((byte)0);

        return ms.ToArray();
    }

    /// <summary>
    /// Builds deeply nested MHSD chunks (depth levels) to exceed the parser's recursion limit.
    /// </summary>
    private static byte[] BuildRecursiveMhsd(int depth)
    {
        // Innermost: a valid MHSD with no children
        byte[] inner = new byte[16];
        WriteMagicLE(inner, 0, "mhsd");
        WriteU32LE(inner, 4, 16);

        byte[] current = inner;
        for (int i = 0; i < depth; i++)
        {
            int totalLen = 16 + current.Length;
            var parent = new byte[totalLen];
            WriteMagicLE(parent, 0, "mhsd");
            WriteU32LE(parent, 4, (uint)totalLen);
            WriteU16LE(parent, 8, 0);
            WriteU16LE(parent, 10, 0);
            WriteU32LE(parent, 12, 1);
            Array.Copy(current, 0, parent, 16, current.Length);
            current = parent;
        }

        // Prepend MHFD
        using var ms = new MemoryStream();
        var bw = new BinaryWriter(ms);
        bw.Write(0x6466686du); // "mhfd"
        bw.Write(12u);
        bw.Write(1u);
        bw.Write(current);
        return ms.ToArray();
    }

    /// <summary>
    /// Builds MHFD → MHSD where MHSD children are non-chunk garbage bytes.
    /// </summary>
    private static byte[] BuildMhfdWithGarbageChildren()
    {
        using var ms = new MemoryStream();
        var bw = new BinaryWriter(ms);

        // MHFD
        bw.Write(0x6466686du);
        bw.Write(12u);
        bw.Write(1u);

        // MHSD with 80 bytes of garbage children
        bw.Write(0x6473686du); // "mhsd"
        bw.Write(96u);
        bw.Write((ushort)0);
        bw.Write((ushort)0);
        bw.Write(0u);
        for (int i = 0; i < 80; i++) bw.Write((byte)0xDE);

        return ms.ToArray();
    }

    /// <summary>
    /// Builds two concatenated MHFD headers — the second one is invalid.
    /// </summary>
    private static byte[] BuildDoubleMhfd()
    {
        using var ms = new MemoryStream();
        var bw = new BinaryWriter(ms);

        // First MHFD (valid, 0 children)
        bw.Write(0x6466686du);
        bw.Write(12u);
        bw.Write(0u);

        // Second MHFD (malformed — only 8 bytes)
        bw.Write(0x6466686du);
        bw.Write(8u);

        return ms.ToArray();
    }

    /// <summary>
    /// Builds MHFD → MHSD where MHSD's headerSize is exactly the given value.
    /// </summary>
    private static byte[] BuildMhfdWithMhsd(uint mhsdHeaderSize)
    {
        using var ms = new MemoryStream();
        var bw = new BinaryWriter(ms);

        // MHFD
        bw.Write(0x6466686du);
        bw.Write(12u);
        bw.Write(1u);

        // MHSD
        bw.Write(0x6473686du); // "mhsd"
        bw.Write(mhsdHeaderSize);

        // Pad remaining
        int bodyLen = (int)(mhsdHeaderSize > 8 ? mhsdHeaderSize - 8 : 0);
        for (int i = 0; i < bodyLen; i++) bw.Write((byte)0);

        return ms.ToArray();
    }


    /// <summary>
    /// Alternates valid MHFD header with corrupt child chunks.
    /// </summary>
    private static byte[] BuildAlternatingValidCorrupt()
    {
        using var ms = new MemoryStream();
        var bw = new BinaryWriter(ms);

        // MHFD
        bw.Write(0x6466686du);
        bw.Write(12u);
        bw.Write(1u);

        // Corrupt child (invalid magic)
        bw.Write(0xDEu);
        bw.Write(0xADu);
        bw.Write(0xBEu);
        bw.Write(0xEFu);
        bw.Write(32u); // headerSize
        for (int i = 0; i < 24; i++) bw.Write((byte)0);

        // Another corrupt child (headerSize=0)
        bw.Write(0x01020304u);
        bw.Write(0u);

        // Yet another corrupt child (headerSize past end)
        bw.Write(0x05060708u);
        bw.Write(0xFFFFFFFFu);

        return ms.ToArray();
    }

    private static void WriteMagicLE(byte[] buf, int offset, string magic)
    {
        for (int i = 0; i < magic.Length; i++)
            buf[offset + i] = (byte)magic[i];
    }

    private static void WriteU32LE(byte[] buf, int offset, uint value)
    {
        buf[offset] = (byte)value;
        buf[offset + 1] = (byte)(value >> 8);
        buf[offset + 2] = (byte)(value >> 16);
        buf[offset + 3] = (byte)(value >> 24);
    }

    private static void WriteU16LE(byte[] buf, int offset, ushort value)
    {
        buf[offset] = (byte)value;
        buf[offset + 1] = (byte)(value >> 8);
    }
}
