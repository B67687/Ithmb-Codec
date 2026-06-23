// SIZE_OK: PhotoDB/ArtworkDB binary chunk parser + data model (~290 LOC)
/*
Photo Database (PhotoDB) and Artwork Database (ArtworkDB) parser for Apple iPod/iPhone
thumbnail cache files. These databases (typically "Photo Database" or "Artwork Database"
in the iPod Photo Cache folder) contain the metadata for .ithmb thumbnail files,
mapping format IDs and dimensions to byte offsets within the .ithmb file.

Format behavior informed by libgpod (db-parse-context.c), iOpenPod, and Keith's
iPod Photo Reader. This is a clean-room implementation for the ithmb-codec plugin.

Chunk structure (iTunesDB-compatible container format):
  MHFD — file header (root container, always first)
  MHSD — section descriptor (indexed sub-container with typed records)
  MHL  — list entry (photo list item)
  MHII — photo/item ID
  MHNI — thumbnail info (format_id + dimensions + .ithmb offset/size)
  MHBA — album container (skipped — not needed for thumbnail extraction)
  MHIA — album item container (skipped)
  MHIF — file info (skipped)
  MHOD — variable-length data record (skipped)

Parse tree for PhotoDB:
  MHFD
  └── MHSD (type=1, "List of Photos")
      └── MHL
          └── MHII
              └── MHSD (type=4, "Thumbnails")
                  └── MHNI ← target: format_id + ithmb_offset + image_size
*/
using System.Globalization;
using System.IO;
using System.Runtime.CompilerServices;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // ============================== Endianness detection ==============================

    /// <summary>Detects file endianness from the MHFD magic bytes.</summary>
    /// <returns>0 for little-endian, 1 for big-endian, -1 if not a valid PhotoDB.</returns>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static int DetectEndianness(ReadOnlySpan<byte> data)
    {
        if (data.Length < 4) return -1;
        // "mhfd" LE: raw bytes are 0x6d, 0x68, 0x66, 0x64
        if (data[0] == 0x6d && data[1] == 0x68 && data[2] == 0x66 && data[3] == 0x64)
            return 0;
        // "dfhm" BE: raw bytes are 0x64, 0x66, 0x68, 0x6d
        if (data[0] == 0x64 && data[1] == 0x66 && data[2] == 0x68 && data[3] == 0x6d)
            return 1;
        return -1;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static bool IsValidPhotoDb(ReadOnlySpan<byte> data) => DetectEndianness(data) >= 0;

    /// <summary>Quick check: do the first 4 bytes spell a valid PhotoDB magic?</summary>
    internal static bool CanOpenPhotoDb(ReadOnlySpan<byte> data)
    {
        if (data.Length < 4) return false;
        // Raw byte check for "mhfd" (LE) or "dfhm" (BE)
        return (data[0] == 0x6d && data[1] == 0x68 && data[2] == 0x66 && data[3] == 0x64)
            || (data[0] == 0x64 && data[1] == 0x66 && data[2] == 0x68 && data[3] == 0x6d);
    }

    // ============================== Known chunk magics (canonical LE uint32) ==============================
    // Each is the uint32 value of the ASCII magic string when read in the file's native endianness.
    // For a LE file, raw bytes match ASCII; for a BE file, raw bytes are byte-swapped but
    // ReadU32BE gives the same canonical value.

    private const uint MagicMhfdLe = 0x6466686d; // "mhfd"
    private const uint MagicMhsdLe = 0x6473686d; // "mhsd"
    private const uint MagicMhlLe  = 0x696c686d; // "mhli"
    private const uint MagicMhiiLe = 0x6969686d; // "mhii"
    private const uint MagicMhbaLe = 0x6162686d; // "mhba"
    private const uint MagicMhiaLe = 0x6169686d; // "mhia"
    private const uint MagicMhifLe = 0x6669686d; // "mhif"
    private const uint MagicMhodLe = 0x646f686d; // "mhod"
    private const uint MagicMhniLe = 0x696e686d; // "mhni"

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static bool IsKnownMagic(uint magicLe)
        => magicLe switch
        {
            MagicMhfdLe or MagicMhsdLe or MagicMhlLe or MagicMhiiLe
                or MagicMhbaLe or MagicMhiaLe or MagicMhifLe or MagicMhodLe
                or MagicMhniLe => true,
            _ => false,
        };

    // ============================== Span-based read helpers ==============================
    // The existing Plugin.cs helpers (ReadU16LE/BE, ReadU32LE/BE) operate on byte[].
    // These span-based versions serve the same purpose for ReadOnlySpan<byte> inputs.

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static uint ReadU32LESpan(ReadOnlySpan<byte> data, int offset) =>
        (uint)(data[offset] | (data[offset + 1] << 8) |
               (data[offset + 2] << 16) | (data[offset + 3] << 24));

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static uint ReadU32BESpan(ReadOnlySpan<byte> data, int offset) =>
        (uint)((data[offset] << 24) | (data[offset + 1] << 16) |
               (data[offset + 2] << 8) | data[offset + 3]);

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static int ReadS32LESpan(ReadOnlySpan<byte> data, int offset) =>
        (int)ReadU32LESpan(data, offset);

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static int ReadS32BESpan(ReadOnlySpan<byte> data, int offset) =>
        (int)ReadU32BESpan(data, offset);

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static ushort ReadU16LESpan(ReadOnlySpan<byte> data, int offset) =>
        (ushort)(data[offset] | (data[offset + 1] << 8));

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static ushort ReadU16BESpan(ReadOnlySpan<byte> data, int offset) =>
        (ushort)((data[offset] << 8) | data[offset + 1]);

    /// <summary>Endian-aware uint32 read.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static uint ReadU32(ReadOnlySpan<byte> data, int offset, int endian) =>
        endian == 0 ? ReadU32LESpan(data, offset) : ReadU32BESpan(data, offset);

    /// <summary>Endian-aware int32 read.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static int ReadS32(ReadOnlySpan<byte> data, int offset, int endian) =>
        endian == 0 ? ReadS32LESpan(data, offset) : ReadS32BESpan(data, offset);

    /// <summary>Endian-aware uint16 read.</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static ushort ReadU16(ReadOnlySpan<byte> data, int offset, int endian) =>
        endian == 0 ? ReadU16LESpan(data, offset) : ReadU16BESpan(data, offset);

    // ============================== Data model structs ==============================

    /// <summary>MHFD — file header, always 12 bytes. Root container of the database.</summary>
    internal readonly struct MhfdHeader
    {
        public readonly uint Magic;
        public readonly uint HeaderSize;   // Always 12 (the size of this header)
        public readonly uint EntryCount;   // Number of top-level MHSD sections

        public MhfdHeader(ReadOnlySpan<byte> data, int offset, int endian)
        {
            Magic = ReadU32(data, offset, endian);
            HeaderSize = ReadU32(data, offset + 4, endian);
            EntryCount = ReadU32(data, offset + 8, endian);
        }
    }

    /// <summary>
    /// MHSD — section descriptor, 16 bytes. Describes a section containing
    /// <see cref="EntryCount"/> records of type <see cref="RecordType"/>.
    /// </summary>
    internal readonly struct MhsdHeader
    {
        public readonly uint Magic;
        public readonly uint HeaderSize;    // Total section size including child entries
        public readonly ushort Index;       // Section index within parent
        public readonly ushort RecordType;  // Type of records: 1=Photos, 4=Thumbnails, etc.
        public readonly uint EntryCount;    // Number of records in this section

        public MhsdHeader(ReadOnlySpan<byte> data, int offset, int endian)
        {
            Magic = ReadU32(data, offset, endian);
            HeaderSize = ReadU32(data, offset + 4, endian);
            Index = ReadU16(data, offset + 8, endian);
            RecordType = ReadU16(data, offset + 10, endian);
            EntryCount = ReadU32(data, offset + 12, endian);
        }
    }

    /// <summary>MHL — photo list entry, 12 bytes. Groups photo items.</summary>
    internal readonly struct MhlHeader
    {
        public readonly uint Magic;
        public readonly uint HeaderSize;
        public readonly uint Count;         // Number of child items

        public MhlHeader(ReadOnlySpan<byte> data, int offset, int endian)
        {
            Magic = ReadU32(data, offset, endian);
            HeaderSize = ReadU32(data, offset + 4, endian);
            Count = ReadU32(data, offset + 8, endian);
        }
    }

    /// <summary>MHII — photo item, 12 bytes. Identifies a single photo.</summary>
    internal readonly struct MhiiHeader
    {
        public readonly uint Magic;
        public readonly uint HeaderSize;
        public readonly uint PhotoId;       // Unique photo identifier

        public MhiiHeader(ReadOnlySpan<byte> data, int offset, int endian)
        {
            Magic = ReadU32(data, offset, endian);
            HeaderSize = ReadU32(data, offset + 4, endian);
            PhotoId = ReadU32(data, offset + 8, endian);
        }
    }

    /// <summary>MHBA — album container, 12 bytes. Skipped (album hierarchy not needed).</summary>
    internal readonly struct MhbaHeader
    {
        public readonly uint Magic;
        public readonly uint HeaderSize;
        public readonly uint AlbumId;       // Unique album identifier

        public MhbaHeader(ReadOnlySpan<byte> data, int offset, int endian)
        {
            Magic = ReadU32(data, offset, endian);
            HeaderSize = ReadU32(data, offset + 4, endian);
            AlbumId = ReadU32(data, offset + 8, endian);
        }
    }

    /// <summary>MHIA — album item container, 12 bytes. Skipped.</summary>
    internal readonly struct MhiaHeader
    {
        public readonly uint Magic;
        public readonly uint HeaderSize;
        public readonly uint ArtworkId;     // Unique artwork identifier

        public MhiaHeader(ReadOnlySpan<byte> data, int offset, int endian)
        {
            Magic = ReadU32(data, offset, endian);
            HeaderSize = ReadU32(data, offset + 4, endian);
            ArtworkId = ReadU32(data, offset + 8, endian);
        }
    }

    /// <summary>MHIF — file info container, 12 bytes. Skipped.</summary>
    internal readonly struct MhifHeader
    {
        public readonly uint Magic;
        public readonly uint HeaderSize;
        public readonly uint InfoType;      // Type of file info

        public MhifHeader(ReadOnlySpan<byte> data, int offset, int endian)
        {
            Magic = ReadU32(data, offset, endian);
            HeaderSize = ReadU32(data, offset + 4, endian);
            InfoType = ReadU32(data, offset + 8, endian);
        }
    }

    /// <summary>
    /// MHOD — variable-length data record, 4-byte header.
    /// Tag=1 indicates a null-terminated string (MhodString).
    /// </summary>
    internal readonly struct MhodHeader
    {
        public readonly ushort Tag;     // 1 = MhodString (null-terminated UTF-16?)
        public readonly ushort Size;    // Size of the data following this header

        public MhodHeader(ReadOnlySpan<byte> data, int offset, int endian)
        {
            Tag = ReadU16(data, offset, endian);
            Size = ReadU16(data, offset + 2, endian);
        }
    }

    /// <summary>
    /// MHNI — thumbnail info entry, 36 bytes.
    /// This is the critical record that maps a format_id to a byte range
    /// within the corresponding .ithmb file.
    /// </summary>
    internal readonly struct MhniHeader
    {
        public readonly uint Magic;
        public readonly uint HeaderSize;    // Always 36
        public readonly int FormatId;       // Matches KnownProfiles keys (e.g. 1019)
        public readonly int ImageSize;      // Byte count of the .ithmb data blob
        public readonly int IthmbOffset;    // Byte offset into the .ithmb file
        public readonly int Width;          // Image width in pixels
        public readonly int Height;         // Image height in pixels
        public readonly int HPadding;       // Horizontal padding (alignment)
        public readonly int VPadding;       // Vertical padding (alignment)

        public MhniHeader(ReadOnlySpan<byte> data, int offset, int endian)
        {
            Magic = ReadU32(data, offset, endian);
            HeaderSize = ReadU32(data, offset + 4, endian);
            FormatId = ReadS32(data, offset + 8, endian);
            ImageSize = ReadS32(data, offset + 12, endian);
            IthmbOffset = ReadS32(data, offset + 16, endian);
            Width = ReadS32(data, offset + 20, endian);
            Height = ReadS32(data, offset + 24, endian);
            HPadding = ReadS32(data, offset + 28, endian);
            VPadding = ReadS32(data, offset + 32, endian);
        }
    }

    // ============================== Parser entry point ==============================

    /// <summary>
    /// Walks a PhotoDB/ArtworkDB binary chunk tree and extracts raw .ithmb data blobs
    /// from all MHNI (thumbnail info) entries found.
    /// </summary>
    /// <param name="data">The full PhotoDB file contents.</param>
    /// <param name="entries">Output list of (format_id, raw ithmb_data) pairs.</param>
    /// <param name="frameCount">Total entries found; equals <c>entries.Count</c>.</param>
    /// <returns>true if the database was valid and parsed successfully; false otherwise.</returns>
    internal static bool TryParsePhotoDb(ReadOnlySpan<byte> data,
        out List<(int FormatId, byte[] Data)> entries, out int frameCount)
    {
        entries = [];
        frameCount = 0;

        int endian = DetectEndianness(data);
        if (endian < 0) return false;

        // Need at least the 12-byte MHFD header
        if (data.Length < 12) return false;

        var mhfd = new MhfdHeader(data, 0, endian);

        // Validate MHFD
        if (mhfd.HeaderSize < 12 || mhfd.Magic != MagicMhfdLe) return false;

        // Walk children starting after the MHFD header
        WalkEntries(data, (int)mhfd.HeaderSize, data.Length, endian, ref entries);
        frameCount = entries.Count;
        return true;
    }

    /// <summary>
    /// Recursive chunk walker. Processes a range of bytes as a sequence of
    /// typed iPod DB chunks, extracting .ithmb data from any MHNI entries.
    /// Container chunks (MHSD, MHII, MHL, MHBA, MHIA) are descended into;
    /// leaf chunks and unknown magics are skipped by headerSize.
    /// </summary>
    private static void WalkEntries(ReadOnlySpan<byte> data, int startOffset, int endOffset,
        int endian, ref List<(int FormatId, byte[] Data)> entries)
    {
        int pos = startOffset;

        while (pos + 8 <= endOffset) // Minimum: magic (4) + headerSize (4)
        {
            uint magicLe = ReadU32(data, pos, endian);
            uint hdrSize = ReadU32(data, pos + 4, endian);

            // Sanity check: headerSize must be >= 8 and within bounds
            if (hdrSize < 8 || pos + hdrSize > endOffset) break;

            if (!IsKnownMagic(magicLe))
            {
                // Unknown chunk — advance by headerSize and continue
                pos += (int)hdrSize;
                continue;
            }

            // ----- MHNI leaf — extract .ithmb blob -----
            if (magicLe == MagicMhniLe)
            {
                if (pos + 36 > endOffset) break;
                var mhni = new MhniHeader(data, pos, endian);

                // Validate offset/size range before slicing
                if (mhni.IthmbOffset >= 0 && mhni.ImageSize > 0 &&
                    mhni.IthmbOffset + mhni.ImageSize <= data.Length)
                {
                    entries.Add((mhni.FormatId, data.Slice(mhni.IthmbOffset, mhni.ImageSize).ToArray()));
                }

                pos += (int)mhni.HeaderSize;
                continue;
            }

            // ----- MHSD — section descriptor, recurse into children -----
            // Fixed header is 16 bytes; children follow immediately after.
            if (magicLe == MagicMhsdLe)
            {
                int childStart = pos + 16;
                int childEnd = pos + (int)hdrSize;
                if (childStart < childEnd)
                    WalkEntries(data, childStart, childEnd, endian, ref entries);
                pos += (int)hdrSize;
                continue;
            }

            // ----- Container types (MHL, MHII) — may contain nested MHSD -----
            // Fixed header is 12 bytes; children follow after the header.
            if (magicLe == MagicMhlLe || magicLe == MagicMhiiLe)
            {
                int childStart = pos + 12;
                int childEnd = pos + (int)hdrSize;
                if (childStart < childEnd)
                    WalkEntries(data, childStart, childEnd, endian, ref entries);
                pos += (int)hdrSize;
                continue;
            }

            // ----- Album hierarchy (MHBA, MHIA) — skipped per spec -----
            // Still descend to find any nested MHNI entries.
            if (magicLe == MagicMhbaLe || magicLe == MagicMhiaLe)
            {
                int childStart = pos + 12;
                int childEnd = pos + (int)hdrSize;
                if (childStart < childEnd)
                    WalkEntries(data, childStart, childEnd, endian, ref entries);
                pos += (int)hdrSize;
                continue;
            }

            // ----- All other known chunks (MHIF, MHOD) — skip -----
            pos += (int)hdrSize;
        }
    }

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

            // Validate header size: must be >= 8 (magic + headerSize)
            if (hdrSize < 8)
            {
                issues.Add($"Invalid chunk header size ({hdrSize}) at offset 0x{pos:x}");
                break;
            }

            long chunkEnd = pos + hdrSize;

            // Check if this is a known magic
            bool known = IsKnownMagic(magicLe);

            // For known chunks, header size must stay within bounds
            if (known && chunkEnd > endOffset)
            {
                issues.Add($"Known chunk at offset 0x{pos:x} with header size {hdrSize} exceeds bounds (end=0x{endOffset:x})");
                break;
            }

            // For unknown data (e.g. inline pixel data in MHSD), silently stop
            // if the purported header exceeds the section boundary.
            if (!known && chunkEnd > endOffset)
                break;

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
                if (childStart < chunkEnd)
                    IntegrityWalkTree(data, childStart, (int)chunkEnd, endian, issues, mhniEntries, ref maxChunkEnd);
                pos = (int)chunkEnd;
                continue;
            }

            // ----- Container types (MHL, MHII) — 12-byte fixed header, recurse -----
            if (magicLe == MagicMhlLe || magicLe == MagicMhiiLe)
            {
                int childStart = pos + 12;
                if (hdrSize < 12)
                {
                    issues.Add($"Container at offset 0x{pos:x} has headerSize < 12 ({hdrSize})");
                    pos = (int)chunkEnd;
                    continue;
                }
                if (childStart < chunkEnd)
                    IntegrityWalkTree(data, childStart, (int)chunkEnd, endian, issues, mhniEntries, ref maxChunkEnd);
                pos = (int)chunkEnd;
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
                if (childStart < chunkEnd)
                    IntegrityWalkTree(data, childStart, (int)chunkEnd, endian, issues, mhniEntries, ref maxChunkEnd);
                pos = (int)chunkEnd;
                continue;
            }

            // ----- All other known chunks (MHIF, MHOD) — skip by headerSize -----
            pos = (int)chunkEnd;
        }
    }

    // ============================== FormatId mapping ==============================

    /// <summary>
    /// Returns a human-readable description for a format_id.
    /// Format IDs map directly to KnownProfiles keys (e.g. 1019 → 720×480 Yuv422 interlaced).
    /// The .ithmb data in PhotoDB/ArtworkDB is raw pixel data (no 4-byte F-prefix header).
    /// </summary>
    internal static string GetFormatIdName(int formatId)
    {
        if (KnownProfiles.TryGetValue(formatId, out var profile))
            return $"{formatId} ({profile.Width}x{profile.Height}, {profile.Encoding})";
        return formatId.ToString(CultureInfo.InvariantCulture);
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

        int mhsdHeaderSize = 16 + count * 36 + totalPixelData;
        int mhsdChildrenStart = 12 + 16; // after MHFD(12) + MHSD(16)

        int[] ithmbOffsets = new int[count];
        int pixelDataStart = mhsdChildrenStart + count * 36;
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

        // Write all MHNI entries
        for (int i = 0; i < count; i++)
        {
            var (formatId, _) = entries[i];
            var profile = KnownProfiles[formatId];

            bw.Write(MagicMhniLe);              // "mhni"
            bw.Write(36u);                      // headerSize
            bw.Write(formatId);                 // formatId
            bw.Write(entries[i].Data.Length);   // imageSize
            bw.Write(ithmbOffsets[i]);          // ithmbOffset
            bw.Write(profile.Width);            // width
            bw.Write(profile.Height);           // height
            bw.Write(0);                        // hPadding
            bw.Write(0);                        // vPadding
        }

        // Write all pixel data blocks
        for (int i = 0; i < count; i++)
            bw.Write(entries[i].Data);

        output = ms.ToArray();
        return true;
    }
}
