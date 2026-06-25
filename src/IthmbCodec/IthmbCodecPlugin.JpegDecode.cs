// JPEG extraction and decode for embedded JPEG payloads in .ithmb files.
// Detects JFIF/Exif markers, carves JPEG slices from byte streams, and
// decodes them via StbImageSharp into BGRA output.
// Separated from plugin ABI glue for independent AOT compilation.

using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using ImageGlass.SDK.Plugins;
using StbImageSharp;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // ------------------------------ JPEG extraction ------------------------------
    internal static bool TryFindJpegSlice(byte[] data, out int offset, out int length, void* cancellation)
    {
        offset = 0; length = 0;
        int i = 0;
        while (i <= data.Length - JpegSoiMarker.Length)
        {
            // SIMD-accelerated search for FF D8
            int soi = data.AsSpan(i).IndexOf(JpegSoiMarker);
            if (soi < 0) return false;
            i += soi;

            // Periodic cancellation check (every 64KB)
            if ((i & 0xFFFF) == 0 && IsCanceled(cancellation)) return false;

            // Validate: FF D8 must be followed by FF (start of a JPEG marker).
            // Reject false positives where random data happens to contain FF D8.
            if (i + 2 >= data.Length || data[i + 2] != 0xFF) { i += JpegSoiMarker.Length; continue; }

            // Verify JFIF or Exif within the scan window (covers marker segments before APP0/APP1)
            int scanEnd = Math.Min(i + JfifExifScanWindow, data.Length);
            int jfifOff = IndexOf(data, JfifMarker, i, scanEnd);
            int exifOff = IndexOf(data, ExifMarker, i, scanEnd);
            if (jfifOff < 0 && exifOff < 0) { i += JpegSoiMarker.Length; continue; }

            offset = i;
            // SIMD-accelerated search for FF D9 after SOI
            int eoiRel = data.AsSpan(offset + JpegSoiMarker.Length).IndexOf(JpegEoiMarker);
            if (eoiRel >= 0)
            {
                length = (offset + JpegSoiMarker.Length + eoiRel + JpegEoiMarker.Length) - offset;
                return true;
            }
            // No EOI found — treat rest of file as the JPEG payload
            length = data.Length - offset;
            return true;
        }
        return false;
    }

    private static IGStatus DecodeJpegSlice(byte[] data, int length, int fileSize,
        void* cancellation, IGImageInfo* outInfo, IGPixelBuffer* outBuf)
    {
        if (IsCanceled(cancellation)) return IGStatus.Canceled;

        if (length <= 0 || length > data.Length)
            return IGStatus.DecodeFailed;

        ImageResult result;
        try
        {
            result = ImageResult.FromMemory(data, ColorComponents.RedGreenBlueAlpha);
        }
        catch (Exception ex)
        {
            Log(4, $"ITHMB: JPEG decode failed ({ex.Message})");
            return IGStatus.DecodeFailed;
        }

        if (result == null || result.Data == null || result.Width <= 0 || result.Height <= 0)
            return IGStatus.DecodeFailed;

        int w = result.Width, h = result.Height;
        int hasAlpha = 0; // stb_image always outputs alpha channel (RGBA) — we treat it as opaque
        FillImageInfo(outInfo, w, h, hasAlpha, ReadExifOrientation(data, 0, length), fileSize);

        if (outBuf == null) return IGStatus.OK; // metadata-only
        if (IsCanceled(cancellation)) return IGStatus.Canceled;

        // Allocate native BGRA buffer and convert RGBA→BGRA
        var allocStatus = AllocateBgraBuffer(w, h, out var stride, out var pixels);
        if (allocStatus != IGStatus.OK) return allocStatus;

        try
        {
            var srcData = result.Data;
            long totalPixels = (long)w * h;
            for (int i = 0; i < totalPixels; i++)
            {
                int si = i * 4;
                pixels[si + 0] = srcData[si + 2]; // B = R
                pixels[si + 1] = srcData[si + 1]; // G = G
                pixels[si + 2] = srcData[si + 0]; // R = B
                pixels[si + 3] = srcData[si + 3]; // A = A
            }
        }
        catch (Exception)
        {
            NativeMemory.Free(pixels);
            return IGStatus.Internal;
        }

        _liveBuffers[(nint)pixels] = 0;
        outBuf->Data = pixels;
        outBuf->Width = w;
        outBuf->Height = h;
        outBuf->Stride = (int)stride;
        outBuf->PixelFormat = (int)IGPixelFormat.Bgra8Unorm;
        return IGStatus.OK;
    }
}
