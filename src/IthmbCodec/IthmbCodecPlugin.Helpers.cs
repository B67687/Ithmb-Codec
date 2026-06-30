// Shared utility helpers for the IthmbCodec plugin: pixel-buffer rotation/allocation,
// EXIF orientation parsing, cancel/log helpers, and size/tolerance constants.
// Separated from the main plugin file for independent compilation and file-size discipline.

using System.Buffers.Binary;
using System.Collections.Concurrent;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using ImageGlass.SDK.Plugins;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // JPEG markers for embedded payload detection
    private static readonly byte[] JfifMarker = "JFIF\0"u8.ToArray();
    private static readonly byte[] ExifMarker = "Exif\0\0"u8.ToArray();
    private static readonly byte[] JpegSoiMarker = [0xFF, 0xD8];
    internal static readonly byte[] JpegEoiMarker = [0xFF, 0xD9];
    private static readonly byte[] App1Marker = [0xFF, 0xE1];

    // Size limit: prevents OOM/DoS on corrupt/malicious files. Largest single raw
    // Largest single frame: 829 KB (P1007 480×864). Largest real .ithmb observed:
    // 852 KB (T-prefix JPEG). No .ithmb file > 1 MB exists in any public repo.
    // Multi-frame concatenation: worst-case 40 frames of the largest profile fit
    // within 32 MB. This covers all plausible real-world usage with generous margin.
    // See research notes in  for the full evidence chain.
    internal const long MaxDecodeFileSize = 32L * 1024 * 1024;
    private const int PeekBufferSize = 4 * 1024 * 1024;           // 4 MB: covers thumbnail JPEG headers + embedded JPEGs
    private const int MaxSignatureProbe = 4096;                    // 4 KB: covers JPEG SOI + marker segments

    // Tolerance for trailing alignment padding bytes. Real .ithmb files from some devices
    // may be slightly smaller than FrameByteLength due to device alignment quirks or
    // incomplete padding. Allow up to 256 bytes slack before rejecting as too small.
    // Based on analysis of iOpenPod's _resolve_packed_geometry trailing-trim approach.
    private const int TrailingPaddingTolerance = 256;

    // JFIF/Exif probe window after SOI (must cover DQT, DHT, COM before APP0/APP1)
    private const int JfifExifScanWindow = 512;

    /// <summary>Rotates a BGRA pixel buffer in place (90, 180, or 270 degrees clockwise). Updates w/h.</summary>
    internal static void RotateBgra(byte* pixels, ref int w, ref int h, int rotation)
    {
        if (rotation == 0) return;
        int srcW = w, srcH = h;
        int pixelCount = srcW * srcH;

        // 180°: simple in-place swap of opposite pixels
        if (rotation == 180)
        {
            for (int i = 0; i < pixelCount / 2; i++)
            {
                int j = pixelCount - 1 - i;
                for (int c = 0; c < 4; c++)
                {
                    (pixels[j * 4 + c], pixels[i * 4 + c]) = (pixels[i * 4 + c], pixels[j * 4 + c]);
                }
            }
            return;
        }

        // 90° or 270°: allocate temp buffer, rotate, copy back
        int dstW = srcH, dstH = srcW;
        int newSize = dstW * dstH * 4;
        byte* rotated = (byte*)NativeMemory.Alloc((nuint)newSize);
        if (rotated == null) return; // OOM: skip rotation
        try
        {
            for (int y = 0; y < srcH; y++)
            {
                for (int x = 0; x < srcW; x++)
                {
                    int srcIdx = (y * srcW + x) * 4;
                    int dstIdx = rotation == 90
                        ? (x * srcH + (srcH - 1 - y)) * 4
                        : ((srcW - 1 - x) * srcH + y) * 4;

                    rotated[dstIdx] = pixels[srcIdx];
                    rotated[dstIdx + 1] = pixels[srcIdx + 1];
                    rotated[dstIdx + 2] = pixels[srcIdx + 2];
                    rotated[dstIdx + 3] = pixels[srcIdx + 3];
                }
            }
            NativeMemory.Copy(rotated, pixels, (nuint)newSize);
            w = dstW;
            h = dstH;
        }
        finally
        {
            // CA1508: rotated is null only when the NativeMemory.Alloc at line 221 failed —
            // we return early in that path, so this finally block is never reached with null
            // after a successful allocation. The early return is the safety guard.
            NativeMemory.Free(rotated);
        }
    }

    /// <summary>Populates an IGImageInfo with common defaults.</summary>
    private static void FillImageInfo(IGImageInfo* info, int w, int h, int hasAlpha, int orientation, long fileSize = -1, int frameCount = 1)
    {
        info->Width = w;
        info->Height = h;
        info->PixelFormat = (int)IGPixelFormat.Bgra8Unorm;
        info->HasAlpha = hasAlpha;
        info->HdrTransferFn = (int)IGHdrTransferFn.None;
        info->ColorSpace = (int)IGColorSpace.Srgb;
        info->Orientation = orientation;
        info->FrameCount = frameCount;
        info->FileSizeBytes = fileSize;
        info->IccProfileData = null;
        info->IccProfileSize = 0;
    }

    /// <summary>Allocates a BGRA8 pixel buffer; returns OOM status on failure.</summary>
    private static IGStatus AllocateBgraBuffer(int w, int h, out ulong stride, out byte* pixels)
    {
        stride = (ulong)w * 4UL;
        ulong size = stride * (ulong)h;
        if (size > int.MaxValue) { pixels = null; return IGStatus.OutOfMemory; }
        pixels = (byte*)NativeMemory.AllocZeroed((nuint)size);
        if (pixels == null) return IGStatus.OutOfMemory;
        return IGStatus.OK;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static int IndexOf(byte[] haystack, byte[] needle, int start, int end)
    {
        int len = end - start;
        return len <= 0 ? -1 : haystack.AsSpan(start, len).IndexOf(needle);
    }

    /// <summary>
    /// Reads the EXIF Orientation tag (0x0112) from a JPEG slice.
    /// Returns 1-8 on success, or 1 (normal) if not found.
    /// </summary>
    internal static int ReadExifOrientation(byte[] data, int jpegOffset, int jpegLength)
    {
        int end = jpegOffset + jpegLength;
        var jpeg = data.AsSpan(jpegOffset, jpegLength);

        // SIMD-accelerated search for APP1 marker (FF E1)
        int app1Rel = jpeg.IndexOf(App1Marker);
        if (app1Rel < 0) return 1;
        int app1Start = jpegOffset + app1Rel;

        // APP1 segment: FF E1 len_len (big-endian 16-bit length including self)
        if (app1Start + 4 >= end) return 1;
        int segEnd = app1Start + 2 + BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(app1Start + 2));
        if (segEnd > end) return 1;

        // Look for "Exif\0\0" header within APP1
        int exifOff = app1Start + 4;
        if (exifOff + 6 > end) return 1;
        if (data[exifOff] != 'E' || data[exifOff + 1] != 'x' ||
            data[exifOff + 2] != 'i' || data[exifOff + 3] != 'f' ||
            data[exifOff + 4] != 0 || data[exifOff + 5] != 0) return 1;

        // TIFF header: "II" (little-endian) or "MM" (big-endian)
        int tiffStart = exifOff + 6;
        if (tiffStart + 8 > end) return 1;
        bool le = data[tiffStart] == 'I' && data[tiffStart + 1] == 'I';
        bool be = data[tiffStart] == 'M' && data[tiffStart + 1] == 'M';
        if (!le && !be) return 1;

        // TIFF magic: 0x002A
        if ((le ? BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(tiffStart + 2))
                : BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(tiffStart + 2))) != 0x002A) return 1;

        // IFD0 offset
        int ifdOff = tiffStart + 4;
        int ifdPos = tiffStart + (int)(le ? BinaryPrimitives.ReadUInt32LittleEndian(data.AsSpan(ifdOff))
                                          : BinaryPrimitives.ReadUInt32BigEndian(data.AsSpan(ifdOff)));
        if (ifdPos < tiffStart + 8 || ifdPos + 2 > end) return 1;

        // Number of IFD entries (16-bit)
        int numEntries = le ? BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(ifdPos))
                           : BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(ifdPos));

        // Scan IFD for Orientation tag (0x0112)
        int entryStart = ifdPos + 2;
        for (int e = 0; e < Math.Min(numEntries, 100) && entryStart + 12 <= end; e++, entryStart += 12)
        {
            int tag = le ? BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(entryStart))
                        : BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(entryStart));
            if (tag != 0x0112) continue;
            // Type must be SHORT (3), count 1
            int type = le ? BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(entryStart + 2))
                         : BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(entryStart + 2));
            int count = (int)(le ? BinaryPrimitives.ReadUInt32LittleEndian(data.AsSpan(entryStart + 4))
                                : BinaryPrimitives.ReadUInt32BigEndian(data.AsSpan(entryStart + 4)));
            if (type != 3 || count != 1) continue;
            // Orientation value is in the last 2 bytes (SHORT fits in 2 bytes)
            int orient = le ? BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(entryStart + 8))
                           : BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(entryStart + 8));
            return orient is >= 1 and <= 8 ? orient : 1;
        }
        return 1;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static bool IsCanceled(void* cancellation)
    {
        // Use cached function pointer (set during init). Avoids chasing _hostApi->Core->{fn} per call.
        return cancellation != null && _isCanceledFn != null && _isCanceledFn(cancellation) != 0;
    }

    private static void Log(int level, string message)
    {
        if (_logFn == null) return;
        fixed (char* p = message) _logFn(level, new IGStringRef { Data = p, Length = message.Length });
    }

    /// <summary>Frees any remaining pixel buffers in _liveBuffers (cleanup guard).</summary>
    private static void FreePixelBufferCleanup()
    {
        foreach (var kv in _liveBuffers)
        {
            NativeMemory.Free((void*)kv.Key);
        }
        _liveBuffers.Clear();
    }
}
