// CL per-pixel nibble-chroma YCbCr 4:2:2 decoder for .ithmb raw profiles.
using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // ---- CL per-pixel nibble chroma (Keith's "CL", Methods 3/4) ----
    //
    // Byte layout per pixel: [Cb:Cr_nibble][Y] — 2 bytes, 1 pixel
    // High nibble = Cb (4-bit, range 0–15), low nibble = Cr (4-bit, range 0–15)
    // Each pixel has independent chroma (not shared like CLCL).
    //
    // Chroma conversion (4-bit → 8-bit): multiply by 16 (shifts nibble to byte range 0–240).
    // Same BT.601 YUV→RGB math as standard YUV422.
    //
    // Confirmed against Keith's iPod Photo Reader source (Methods 3 and 4).
    internal static bool DecodeYuv422Cl(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        if (w <= 0 || h <= 0) return false;
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return false;

        int rowStride = (int)((long)src.Length / h);
        for (int y = 0; y < h; y++)
        {
            int rowStart = y * rowStride;
            byte* pDstRow = dst + (nint)(y * w * 4);
            for (int x = 0; x < w; x++)
            {
                int idx = rowStart + x * 2;
                int packed = src[idx];
                int cb = ((packed >> 4) & 0x0F) * 16 - 128;
                int cr = (packed & 0x0F) * 16 - 128;
                int yy = src[idx + 1];

                WriteYuvPixel(pDstRow, yy, cb, cr);
                pDstRow += 4;
            }
        }
        return true;
    }
}
