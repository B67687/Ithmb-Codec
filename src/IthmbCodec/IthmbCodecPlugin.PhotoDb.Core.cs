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
    /// MHNI — thumbnail info entry, 36 bytes (iPod Classic) or 76 bytes (Apple TV/Animal).
    /// This is the critical record that maps a format_id to a byte range
    /// within the corresponding .ithmb file.
    /// </summary>
    internal readonly struct MhniHeader
    {
        public readonly uint Magic;
        public readonly uint HeaderSize;    // 36 (classic) or 76 (Apple TV/Animal)
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

            // Field layout verified against iPod Classic 6G (Reuhno's ArtworkDB) and
            // Apple TV / Animal PhotoDB. Both use 76-byte headers but with different
            // field offsets and semantics.
            //
            // iPod Classic 76B layout (verified):
            //   +16 (0x10): FormatId (u32)
            //   +20 (0x14): IthmbOffset (u32)
            //   +24 (0x18): ImageSize (u32)
            //   +32 (0x20): Height (s16)
            //   +34 (0x22): Width  (s16)
            //
            // Apple TV / Animal 76B layout:
            //   Same header size but values point to external F{id}_1.ithmb files.
            //   Width/Height packed at +20.
            //
            // Detect variant: if IthmbOffset at +20 is reasonable (< data length)
            // and ImageSize at +24 is > 0, treat as inline (iPod Classic).
            // Otherwise mark as external (Apple TV / Animal).

            int fmtId = ReadS32(data, offset + 16, endian);
            int ithmbOff = ReadS32(data, offset + 20, endian);
            int imgSize = ReadS32(data, offset + 24, endian);

            // Validate IthmbOffset: must be non-negative and within data bounds
            bool isInline = ithmbOff >= 0 && imgSize > 0
                            && (long)ithmbOff + imgSize <= data.Length;

            if (isInline)
            {
                // iPod Classic 6G/7G — inline data in .ithmb files
                FormatId = fmtId;
                IthmbOffset = ithmbOff;
                ImageSize = imgSize;
                Width = (short)ReadU16(data, offset + 34, endian);
                Height = (short)ReadU16(data, offset + 32, endian);
            }
            else
            {
                // Apple TV / Animal — external .ithmb files or no data
                FormatId = fmtId;
                ImageSize = 0;
                IthmbOffset = -1;
                int packed = ReadS32(data, offset + 20, endian);
                Width = packed & 0xFFFF;
                Height = (packed >> 16) & 0xFFFF;
            }
            HPadding = 0;
            VPadding = 0;
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
        out List<(int FormatId, byte[] Data, int IthmbOffset, int ImageSize)> entries, out int frameCount)
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

        for (int i = 0; i < entries.Count; i++)
        {
            var entry = entries[i];
            if (KnownProfiles.ContainsKey(entry.FormatId)) continue;
            if (entry.Data.Length >= 2 && entry.Data[0] == 0xFF && entry.Data[1] == 0xD8)
            {
                int eoiRel = entry.Data.AsSpan(2).IndexOf(JpegEoiMarker);
                if (eoiRel >= 0)
                {
                    int jpegLen = 2 + eoiRel + JpegEoiMarker.Length;
                    if (jpegLen < entry.Data.Length)
                    {
                        var trimmed = new byte[jpegLen];
                        Array.Copy(entry.Data, trimmed, jpegLen);
                        entries[i] = (entry.FormatId, trimmed, entry.IthmbOffset, jpegLen);
                    }
                }
            }
        }

        frameCount = entries.Count;
        return true;
    }

    /// <summary>
    /// Recursive chunk walker. Processes a range of bytes as a sequence of
    /// typed iPod DB chunks, extracting .ithmb data from any MHNI entries.
    /// Container chunks (MHSD, MHII, MHL, MHBA, MHIA) are descended into;
    /// leaf chunks and unknown magics are skipped by headerSize.
    /// </summary>
    /// <summary>Validates that a range begins with at least one valid known chunk.</summary>
    /// <remarks>Apple TV and Animal PhotoDB files may use MHSD/MHLI as leaf chunks
    /// whose body contains entry data rather than child chunks. This gate prevents
    /// recursing into non-container body data.</remarks>
    private static bool HasChildChunks(ReadOnlySpan<byte> data, int start, int end, int endian)
    {
        if (start + 8 > end) return false;
        uint magicLe = ReadU32(data, start, endian);
        uint hdrSize = ReadU32(data, start + 4, endian);
        if (hdrSize < 8 || start + hdrSize > end) return false;
        return IsKnownMagic(magicLe);
    }

    private static void WalkEntries(ReadOnlySpan<byte> data, int startOffset, int endOffset,
        int endian, ref List<(int FormatId, byte[] Data, int IthmbOffset, int ImageSize)> entries)
    {
        int pos = startOffset;

        while (pos + 8 <= endOffset) // Minimum: magic (4) + headerSize (4)
        {
            uint magicLe = ReadU32(data, pos, endian);
            uint hdrSize = ReadU32(data, pos + 4, endian);

            // Sanity check: headerSize must be >= 8 and within bounds
            if (hdrSize < 8 || pos + hdrSize > endOffset)
            {
                // Unknown bytes or padding between chunks — advance by 1 and re-scan.
                // Breaking here would miss subsequent chunks (e.g. MHNI after padding).
                pos++;
                continue;
            }

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
                    (long)mhni.IthmbOffset + mhni.ImageSize <= data.Length)
                {
                    entries.Add((mhni.FormatId, data.Slice(mhni.IthmbOffset, mhni.ImageSize).ToArray(), mhni.IthmbOffset, mhni.ImageSize));
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
                if (childStart < childEnd && HasChildChunks(data, childStart, childEnd, endian))
                    WalkEntries(data, childStart, childEnd, endian, ref entries);
                pos += (int)hdrSize;
                continue;
            }

            // ----- MHL — photo list, fixed 12-byte header -----
            // Total atom size is at offset +4 (hdrSize). Children follow the 12-byte header.
            if (magicLe == MagicMhlLe)
            {
                int childStart = pos + 12;
                int childEnd = pos + (int)hdrSize;
                if (childStart < childEnd && HasChildChunks(data, childStart, childEnd, endian))
                    WalkEntries(data, childStart, childEnd, endian, ref entries);
                pos += (int)hdrSize;
                continue;
            }

            // ----- MHII — photo/item ID, variable-length header (typically 152 bytes) -----
            // hdrSize at offset +4 is the header length (NOT total atom size).
            // Total atom size is at offset +8 (total_len). Children start at pos + hdrSize.
            if (magicLe == MagicMhiiLe)
            {
                uint totalLen = ReadU32(data, pos + 8, endian);
                int childStart = pos + (int)hdrSize;
                int childEnd = pos + (int)totalLen;
                if (childStart < childEnd && HasChildChunks(data, childStart, childEnd, endian))
                    WalkEntries(data, childStart, childEnd, endian, ref entries);
                pos += (int)totalLen;
                continue;
            }

            // ----- Album hierarchy (MHBA, MHIA) — skipped per spec -----
            // Still descend to find any nested MHNI entries.
            if (magicLe == MagicMhbaLe || magicLe == MagicMhiaLe)
            {
                int childStart = pos + 12;
                int childEnd = pos + (int)hdrSize;
                if (childStart < childEnd && HasChildChunks(data, childStart, childEnd, endian))
                    WalkEntries(data, childStart, childEnd, endian, ref entries);
                pos += (int)hdrSize;
                continue;
            }

            // ----- All other known chunks (MHIF, MHOD) — skip -----
            pos += (int)hdrSize;
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
}
