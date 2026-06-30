// PhotoDB/ArtworkDB binary chunk parser: endian detection, tree walker, and entry extraction.
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

using IthmbCodec;

namespace IthmbCodec.PhotoDb;

internal static unsafe partial class PhotoDb
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


    // ============================== Parser entry point ==============================

    /// <summary>
    /// Walks a PhotoDB/ArtworkDB binary chunk tree and extracts raw .ithmb data blobs
    /// from all MHNI (thumbnail info) entries found.
    /// </summary>
    /// <param name="data">The full PhotoDB file contents.</param>
    /// <param name="entries">Output list of (format_id, raw ithmb_data, ithmb_offset, image_size, width, height) pairs.</param>
    /// <param name="frameCount">Total entries found; equals <c>entries.Count</c>.</param>
    /// <returns>true if the database was valid and parsed successfully; false otherwise.</returns>
    internal static bool TryParsePhotoDb(ReadOnlySpan<byte> data,
        out List<(int FormatId, byte[] Data, int IthmbOffset, int ImageSize, int Width, int Height)> entries, out int frameCount)
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
            if (IthmbCodecPlugin.KnownProfiles.ContainsKey(entry.FormatId)) continue;
            if (entry.Data.Length >= 2 && entry.Data[0] == 0xFF && entry.Data[1] == 0xD8)
            {
                int eoiRel = entry.Data.AsSpan(2).IndexOf(IthmbCodecPlugin.JpegEoiMarker);
                if (eoiRel >= 0)
                {
                    int jpegLen = 2 + eoiRel + IthmbCodecPlugin.JpegEoiMarker.Length;
                    if (jpegLen < entry.Data.Length)
                    {
                        var trimmed = new byte[jpegLen];
                        Array.Copy(entry.Data, trimmed, jpegLen);
                        entries[i] = (entry.FormatId, trimmed, entry.IthmbOffset, jpegLen, entry.Width, entry.Height);
                    }
                }
            }
        }

        frameCount = entries.Count;
        return true;
    }

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
        int endian, ref List<(int FormatId, byte[] Data, int IthmbOffset, int ImageSize, int Width, int Height)> entries,
        int depth = 0)
    {
        if (depth > 64) return;
        int pos = startOffset;

        while (pos + 8 <= endOffset) // Minimum: magic (4) + headerSize (4)
        {
            uint magicLe = ReadU32(data, pos, endian);
            uint hdrSize = ReadU32(data, pos + 4, endian);

            // Sanity check: headerSize must be >= 8 and within bounds
            if (hdrSize < 8 || (long)pos + hdrSize > endOffset)
            {
                // Unknown bytes or padding between chunks — advance by 1 and re-scan.
                // Breaking here would miss subsequent chunks (e.g. MHNI after padding).
                pos += hdrSize == 0 ? 4 : 1;
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
                    entries.Add((mhni.FormatId, data.Slice(mhni.IthmbOffset, mhni.ImageSize).ToArray(), mhni.IthmbOffset, mhni.ImageSize, mhni.Width, mhni.Height));
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
                    WalkEntries(data, childStart, childEnd, endian, ref entries, depth + 1);
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
                    WalkEntries(data, childStart, childEnd, endian, ref entries, depth + 1);
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
                    WalkEntries(data, childStart, childEnd, endian, ref entries, depth + 1);
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
                    WalkEntries(data, childStart, childEnd, endian, ref entries, depth + 1);
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
        if (IthmbCodecPlugin.KnownProfiles.TryGetValue(formatId, out var profile))
            return $"{formatId} ({profile.Width}x{profile.Height}, {profile.Encoding})";
        return formatId.ToString(CultureInfo.InvariantCulture);
    }
}
