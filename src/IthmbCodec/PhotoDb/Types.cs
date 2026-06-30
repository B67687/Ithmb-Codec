// PhotoDB/ArtworkDB binary chunk types: magic constants, endian-aware span read
// helpers, and all chunk header structs. Separated from the parser for file-size discipline.

using System.Runtime.CompilerServices;

namespace IthmbCodec.PhotoDb;

internal static unsafe partial class PhotoDb
{
    // ============================== Known chunk magics (canonical LE uint32) ==============================
    // Each is the uint32 value of the ASCII magic string when read in the file's native endianness.
    // For a LE file, raw bytes match ASCII; for a BE file, raw bytes are byte-swapped but
    // ReadU32BE gives the same canonical value.

    private const uint MagicMhfdLe = 0x6466686d; // "mhfd"
    private const uint MagicMhsdLe = 0x6473686d; // "mhsd"
    private const uint MagicMhlLe = 0x696c686d; // "mhli"
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
}
