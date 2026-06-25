// CLCL nibble-chroma YCbCr 4:2:2 decoder for .ithmb raw profiles.
using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // ---------- CLCL nibble-chroma (speculative, untested) ----------

    /// <summary>
    /// Decodes CLCL-packed YCbCr 4:2:2: one chroma byte packs Cb (high nibble) and
    /// Cr (low nibble) at 4-bit precision. Two luma bytes follow for two pixels.
    /// Byte layout per macropixel: [CbCr] [Y0] [CbCr] [Y1]  —  4 bytes, 2 pixels.
    /// The two CbCr bytes are identical (same packed chroma for both pixels).
    ///
    /// Chroma conversion (4-bit → 8-bit): multiply by 16 (shifts nibble to byte range 0-240).
    /// Confirmed against andrewmalta/ithmb and wrinklykong/pyithmb sources.
    /// Keith Wiley method 1 uses full 8-bit chroma (different variant, no nibble packing).
    /// Same BT.601 YUV→RGB math as standard YUV422.
    ///
    /// SPECULATIVE — no real-world .ithmb sample files available for verification.
    /// The neutral-chroma unit test validates the math but not real file compatibility.
    /// Based on andrewmalta/ithmb, wrinklykong/pyithmb, and Keith's iPod Photo Reader.
    /// Activate via profiles.json for iPod 4G/5G files that decode incorrectly
    /// with the standard UYVY path.
    /// </summary>
    internal static bool DecodeYuv422Clcl(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        if (w <= 0 || h <= 0) return false;
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return false;
        if ((w & 1) != 0) return false; // pair processing requires even width

        int rowStride = (int)((long)src.Length / h);
        for (int y = 0; y < h; y++)
        {
            int rowStart = y * rowStride;
            byte* pDstRow = dst + (nint)(y * w * 4);
            for (int x = 0; x < w; x += 2)
            {
                int idx = rowStart + x * 2;
                int packed = src[idx];
                int cb = ((packed >> 4) & 0x0F) * 16 - 128;
                int cr = (packed & 0x0F) * 16 - 128;
                int y0 = src[idx + 1];
                int y1 = src[idx + 3];

                WriteYuvPixel(pDstRow, y0, cb, cr);
                if (x + 1 < w) WriteYuvPixel(pDstRow + 4, y1, cb, cr);
                pDstRow += 8;
            }
        }
        return true;
    }
}
