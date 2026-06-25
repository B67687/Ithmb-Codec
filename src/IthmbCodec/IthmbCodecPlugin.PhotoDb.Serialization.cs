using System.IO;
using System.Runtime.CompilerServices;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // ============================== Integrity check ==============================

    /// <summary>
    /// Validates a PhotoDB/ArtworkDB binary for structural integrity.
    /// Checks: known chunk magics, non-overlapping MHNI entries, known format IDs,
    /// valid range bounds, no trailing garbage.
    /// Returns a list of issue descriptions (empty = clean).
    /// </summary>
    internal static List<string> IntegrityCheckPhotoDb(ReadOnlySpan<byte> data)
    {
        var issues = new List<string>();

        // 1. Minimum size check
        if (data.Length < 4)
        {
            issues.Add("File too small (< 4 bytes)");
            return issues;
        }

        // 2. Magic check
        if (!CanOpenPhotoDb(data))
        {
            issues.Add("Not a valid PhotoDB/ArtworkDB file (bad magic)");
            return issues;
        }

        // 3. Endianness
        int endian = DetectEndianness(data);
        if (endian < 0)
        {
            issues.Add("Cannot detect endianness");
            return issues;
        }

        // 4. Try full parse — note failure but continue structural check
        bool parseOk = TryParsePhotoDb(data, out _, out _);
        if (!parseOk)
        {
            issues.Add("TryParsePhotoDb failed — structural issue during full parse");
        }

        // 5. Validate MHFD header
        if (data.Length < 12)
        {
            issues.Add("File too small for MHFD header (< 12 bytes)");
            return issues;
        }

        var mhfd = new MhfdHeader(data, 0, endian);
        if (mhfd.HeaderSize < 12)
        {
            issues.Add("MHFD header size is invalid (< 12)");
        }

        int mhfdSize = (int)mhfd.HeaderSize;
        if (mhfdSize > data.Length)
        {
            issues.Add($"MHFD header size ({mhfdSize}) exceeds file size ({data.Length})");
            mhfdSize = data.Length;
        }

        // Track MHNI entries and chunk boundaries
        var mhniEntries = new List<(int FormatId, int IthmbOffset, int ImageSize, int ChunkOffset)>();
        int maxChunkEnd = mhfdSize;

        // Walk chunk tree from MHFD end
        IntegrityWalkTree(data, mhfdSize, data.Length, endian, issues, mhniEntries, ref maxChunkEnd);

        // 6. Validate known format IDs for all MHNI entries
        foreach (var entry in mhniEntries)
        {
            if (!KnownProfiles.ContainsKey(entry.FormatId))
            {
                issues.Add($"Format ID {entry.FormatId} not found in KnownProfiles (at chunk offset 0x{entry.ChunkOffset:x})");
            }
        }

        // 7. Check for overlapping MHNI ithmb offset ranges
        for (int i = 0; i < mhniEntries.Count; i++)
        {
            var a = mhniEntries[i];
            for (int j = i + 1; j < mhniEntries.Count; j++)
            {
                var b = mhniEntries[j];
                if (a.IthmbOffset < b.IthmbOffset + b.ImageSize &&
                    b.IthmbOffset < a.IthmbOffset + a.ImageSize)
                {
                    issues.Add($"Overlapping ithmb offset ranges: entry at 0x{a.ChunkOffset:x} (offset={a.IthmbOffset}, size={a.ImageSize}) overlaps with entry at 0x{b.ChunkOffset:x} (offset={b.IthmbOffset}, size={b.ImageSize})");
                }
            }
        }

        // 8. Check for trailing garbage after the last known chunk boundary
        if (maxChunkEnd < data.Length)
        {
            issues.Add($"Trailing garbage detected: {data.Length - maxChunkEnd} byte(s) after last known chunk boundary");
        }

        return issues;
    }

    /// <summary>
    /// Recursive chunk walker for integrity checking. Validates chunk structure,
    /// collects MHNI entries, and tracks the furthest chunk boundary.
    /// </summary>
    private static void IntegrityWalkTree(ReadOnlySpan<byte> data, int startOffset, int endOffset,
        int endian, List<string> issues,
        List<(int FormatId, int IthmbOffset, int ImageSize, int ChunkOffset)> mhniEntries,
        ref int maxChunkEnd)
    {
        int pos = startOffset;

        while (pos + 8 <= endOffset)
        {
            uint magicLe = ReadU32(data, pos, endian);
            uint hdrSize = ReadU32(data, pos + 4, endian);
            bool known = IsKnownMagic(magicLe);

            // Validate header size: must be >= 8 (magic + headerSize)
            // Padding bytes between known chunks (e.g. zeros after MHOD before MHNI) may
            // appear as unknown data with hdrSize=0. Skip them instead of breaking.
            if (hdrSize < 8)
            {
                if (!known)
                {
                    pos++;
                    continue;
                }
                issues.Add($"Invalid chunk header size ({hdrSize}) at offset 0x{pos:x}");
                break;
            }

            long chunkEnd = pos + hdrSize;

            // For known chunks, header size must stay within bounds
            if (known && chunkEnd > endOffset)
            {
                issues.Add($"Known chunk at offset 0x{pos:x} with header size {hdrSize} exceeds bounds (end=0x{endOffset:x})");
                break;
            }

            // For unknown data that exceeds the section boundary, it's likely
            // padding between known chunks — advance by 1 and re-scan.
            if (!known && chunkEnd > endOffset)
            {
                pos++;
                continue;
            }

            // Track furthest chunk boundary
            if (chunkEnd > maxChunkEnd)
                maxChunkEnd = (int)chunkEnd;

            if (!known)
            {
                // Unknown chunk — advance by headerSize and continue
                pos = (int)chunkEnd;
                continue;
            }

            // ----- MHNI leaf — validate and collect -----
            if (magicLe == MagicMhniLe)
            {
                if (pos + 36 > data.Length)
                {
                    issues.Add($"MHNI at offset 0x{pos:x} truncated: need 36 bytes, have {data.Length - pos}");
                    break;
                }

                var mhni = new MhniHeader(data, pos, endian);

                if (mhni.HeaderSize < 36)
                {
                    issues.Add($"MHNI at offset 0x{pos:x} has headerSize < 36 ({mhni.HeaderSize})");
                }

                if (mhni.IthmbOffset < 0)
                {
                    issues.Add($"MHNI at offset 0x{pos:x} has negative ithmbOffset ({mhni.IthmbOffset})");
                }
                if (mhni.ImageSize < 0)
                {
                    issues.Add($"MHNI at offset 0x{pos:x} has negative imageSize ({mhni.ImageSize})");
                }
                if (mhni.IthmbOffset >= 0 && mhni.ImageSize >= 0 &&
                    mhni.IthmbOffset + mhni.ImageSize > data.Length)
                {
                    issues.Add($"MHNI at offset 0x{pos:x}: ithmbOffset ({mhni.IthmbOffset}) + imageSize ({mhni.ImageSize}) = {mhni.IthmbOffset + mhni.ImageSize} exceeds data length ({data.Length})");
                }

                mhniEntries.Add((mhni.FormatId, mhni.IthmbOffset, mhni.ImageSize, pos));
                pos = (int)chunkEnd;
                continue;
            }

            // ----- MHSD — section descriptor, recurse into children -----
            if (magicLe == MagicMhsdLe)
            {
                int childStart = pos + 16;
                if (hdrSize < 16)
                {
                    issues.Add($"MHSD at offset 0x{pos:x} has headerSize < 16 ({hdrSize})");
                    pos = (int)chunkEnd;
                    continue;
                }
                if (childStart < chunkEnd && HasChildChunks(data, childStart, (int)chunkEnd, endian))
                    IntegrityWalkTree(data, childStart, (int)chunkEnd, endian, issues, mhniEntries, ref maxChunkEnd);
                pos = (int)chunkEnd;
                continue;
            }

            // ----- MHL — photo list, fixed 12-byte header -----
            // hdrSize at +4 is the total atom size. Children follow the 12-byte header.
            if (magicLe == MagicMhlLe)
            {
                int childStart = pos + 12;
                if (hdrSize < 12)
                {
                    issues.Add($"MHL at offset 0x{pos:x} has headerSize < 12 ({hdrSize})");
                    pos = (int)chunkEnd;
                    continue;
                }
                if (childStart < chunkEnd && HasChildChunks(data, childStart, (int)chunkEnd, endian))
                    IntegrityWalkTree(data, childStart, (int)chunkEnd, endian, issues, mhniEntries, ref maxChunkEnd);
                pos = (int)chunkEnd;
                continue;
            }

            // ----- MHII — photo/item ID, variable-length header -----
            // hdrSize at +4 is the header length (NOT total atom size).
            // Total atom size is at offset +8 (total_len). Children start at pos + hdrSize.
            if (magicLe == MagicMhiiLe)
            {
                uint totalLen = ReadU32(data, pos + 8, endian);
                long mhiiEnd = pos + totalLen;
                int childStart = pos + (int)hdrSize;
                if (hdrSize < 12)
                {
                    issues.Add($"MHII at offset 0x{pos:x} has headerSize < 12 ({hdrSize})");
                    pos = (int)mhiiEnd;
                    continue;
                }
                if (childStart < mhiiEnd && HasChildChunks(data, childStart, (int)mhiiEnd, endian))
                    IntegrityWalkTree(data, childStart, (int)mhiiEnd, endian, issues, mhniEntries, ref maxChunkEnd);
                pos = (int)mhiiEnd;
                continue;
            }

            // ----- Album containers (MHBA, MHIA) — 12-byte fixed header, recurse -----
            if (magicLe == MagicMhbaLe || magicLe == MagicMhiaLe)
            {
                int childStart = pos + 12;
                if (hdrSize < 12)
                {
                    issues.Add($"Album container at offset 0x{pos:x} has headerSize < 12 ({hdrSize})");
                    pos = (int)chunkEnd;
                    continue;
                }
                if (childStart < chunkEnd && HasChildChunks(data, childStart, (int)chunkEnd, endian))
                    IntegrityWalkTree(data, childStart, (int)chunkEnd, endian, issues, mhniEntries, ref maxChunkEnd);
                pos = (int)chunkEnd;
                continue;
            }

            // ----- All other known chunks (MHIF, MHOD) — skip by headerSize -----
            pos = (int)chunkEnd;
        }
    }

    // ============================== PhotoDB builder ==============================

    /// <summary>
    /// Builds a synthetic PhotoDB/ArtworkDB binary from a list of format entries.
    /// Creates minimal valid chunk tree: MHFD → MHSD → MHNI per entry → raw pixel data.
    /// </summary>
    /// <param name="entries">List of (FormatId, raw_ithmb_data) — the same type used in TryParsePhotoDb output.</param>
    /// <param name="output">The complete PhotoDB binary if successful.</param>
    /// <returns>true if the database was built successfully; false if entries is empty, an unknown format ID, or size mismatch.</returns>
    internal static bool TryBuildPhotoDb(List<(int FormatId, byte[] Data)> entries, out byte[] output)
    {
        output = null!;

        // Guard: null or empty
        if (entries == null || entries.Count == 0)
            return false;

        int count = entries.Count;

        // Guard: validate all format IDs and data sizes
        for (int i = 0; i < count; i++)
        {
            var (formatId, data) = entries[i];
            if (!KnownProfiles.TryGetValue(formatId, out var profile))
                return false;
            if (data == null || data.Length != profile.FrameByteLength)
                return false;
        }

        // Precompute ithmbOffset for each entry: all MHNI entries are grouped
        // together, followed by all pixel data blocks. The parser reads chunks
        // sequentially; pixel data after the last MHNI is skipped by the unknown-
        // magic fallthrough.
        // Layout: [MHFD 12][MHSD 16][MHNI(0)..MHNI(N-1)][pixels(0)..pixels(N-1)]

        int totalPixelData = 0;
        for (int i = 0; i < count; i++)
            totalPixelData += entries[i].Data.Length;

        int mhniTotalLen = 76 + 64; // 76-byte header + padding/filename area
        int mhsdHeaderSize = 16 + count * mhniTotalLen + totalPixelData;
        int mhsdChildrenStart = 12 + 16; // after MHFD(12) + MHSD(16)

        int[] ithmbOffsets = new int[count];
        int pixelDataStart = mhsdChildrenStart + count * mhniTotalLen;
        for (int i = 0; i < count; i++)
        {
            ithmbOffsets[i] = pixelDataStart;
            pixelDataStart += entries[i].Data.Length;
        }

        using var ms = new MemoryStream();
        var bw = new BinaryWriter(ms);

        // MHFD header (12 bytes LE)
        bw.Write(MagicMhfdLe);  // "mhfd"
        bw.Write(12u);          // headerSize
        bw.Write(1u);           // entryCount (one MHSD section)

        // MHSD section header (16 bytes LE)
        bw.Write(MagicMhsdLe);  // "mhsd"
        bw.Write((uint)mhsdHeaderSize);
        bw.Write((ushort)0);    // index
        bw.Write((ushort)4);    // recordType = 4 (thumbnails)
        bw.Write((uint)count);  // entryCount

        // Write all MHNI entries (76-byte iPod Classic layout)
        // mhniTotalLen defined above
        for (int i = 0; i < count; i++)
        {
            var (formatId, _) = entries[i];
            var profile = KnownProfiles[formatId];

            bw.Write(MagicMhniLe);              // "mhni"
            bw.Write(76u);                      // headerSize
            bw.Write((uint)mhniTotalLen);       // total_len at +8
            bw.Write(1u);                       // entryIndex at +12
            bw.Write(formatId);                 // formatId at +16
            bw.Write(ithmbOffsets[i]);          // ithmbOffset at +20
            bw.Write(entries[i].Data.Length);   // imageSize at +24
            bw.Write(0u);                       // padding at +28
            bw.Write((short)profile.Height);    // height at +32
            bw.Write((short)profile.Width);     // width at +34
            for (int j = 0; j < mhniTotalLen - 36; j++) bw.Write((byte)0); // padding/filename
        }

        // Write all pixel data blocks
        for (int i = 0; i < count; i++)
            bw.Write(entries[i].Data);

        output = ms.ToArray();
        return true;
    }
}
