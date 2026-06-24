// Decode algorithms for .ithmb raw profiles — CLCL/CL nibble-chroma, YCbCr 4:2:0, shared utilities.
// Separated from plugin ABI glue for independent AOT compilation.
using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Intrinsics;
using System.Runtime.Intrinsics.Arm;
using System.Runtime.Intrinsics.X86;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // SIZE_OK: CLCL/CL decoders + YCbCr420 + shared SIMD tail/utility handlers (~250 LOC)

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

    // ---------- YCbCr 4:2:0 (planar) ----------

    /// <summary>Returns false when the input buffer is too small (defensive guard).</summary>
    internal static bool DecodeYcbcr420(ReadOnlySpan<byte> src, byte* dst, int w, int h,
        bool swapChromaPlanes = false)
    {
        if (w <= 0 || h <= 0) return false;
        long totalPixels = (long)w * h;
        int ySize = (int)totalPixels; // ≤ MaxDecodeFileSize, safe for int
        int uvSize = ((w + 1) / 2) * ((h + 1) / 2);
        long expectedBytes = totalPixels + (long)uvSize * 2;
        if (src.Length < expectedBytes) return false;

        // SIMD path: requires even dimensions (2×2 blocks fit perfectly in Vector128<int>)
        // Note: SIMD path always uses standard Cb/Cr order (swapChromaPlanes not supported there)
        if (!swapChromaPlanes && Vector128.IsHardwareAccelerated && (w & 1) == 0 && (h & 1) == 0)
            DecodeYcbcr420_SIMD(src, dst, w, h, ySize, uvSize);
        else
            DecodeYcbcr420_Scalar(src, dst, w, h, ySize, uvSize, swapChromaPlanes);
        return true;
    }

    /// <summary>Scalar fallback for YCbCr 4:2:0 (extracted from original body).</summary>
    private static void DecodeYcbcr420_Scalar(ReadOnlySpan<byte> src, byte* dst,
        int w, int h, int ySize, int uvSize, bool swapChromaPlanes = false)
    {
        int uvStride = (w + 1) / 2;
        for (int y = 0; y < h; y += 2)
        {
            for (int x = 0; x < w; x += 2)
            {
                int uvIdx = ySize + (y / 2) * uvStride + (x / 2);
                int cb, cr;
                if (swapChromaPlanes)
                {
                    cr = src[uvIdx] - 128;
                    cb = src[uvIdx + uvSize] - 128;
                }
                else
                {
                    cb = src[uvIdx] - 128;
                    cr = src[uvIdx + uvSize] - 128;
                }

                for (int dy = 0; dy < 2 && y + dy < h; dy++)
                {
                    int yRowStart = (y + dy) * w;
                    byte* pDstBlock = dst + (y + dy) * w * 4 + x * 4;
                    for (int dx = 0; dx < 2 && x + dx < w; dx++)
                    {
                        int yy = src[yRowStart + x + dx];
                        WriteYuvPixel(pDstBlock, yy, cb, cr);
                        pDstBlock += 4;
                    }
                }
            }
        }
    }

    /// <summary>Vector128-accelerated YCbCr 4:2:0 → BGRA.</summary>
    /// <remarks>
    /// Processes one 2×2 block per iteration. 4 luma values + shared Cb/Cr
    /// map directly to Vector128&lt;int&gt; (4 × int32 lanes).
    /// Cross-platform: SSE2 on x64, NEON on ARM64.
    /// </remarks>
    private static void DecodeYcbcr420_SIMD(ReadOnlySpan<byte> src, byte* dst,
        int w, int h, int ySize, int uvSize)
    {
        int uvStride = (w + 1) / 2;

        // Loop-invariant constant vectors
        var zero = Vector128<int>.Zero;
        var maxVal = Vector128.Create(255);
        var alpha = Vector128.Create(255 << 24);
        var rCoef = Vector128.Create(YuvRCoef);
        var gCoefCb = Vector128.Create(YuvGCoefCb);
        var gCoefCr = Vector128.Create(YuvGCoefCr);
        var bCoef = Vector128.Create(YuvBCoef);

        for (int y = 0; y < h; y += 2)
        {
            int yRow0 = y * w;
            int yRow1 = (y + 1) * w;
            byte* dstRow0 = dst + (nint)(y * w * 4);
            byte* dstRow1 = dst + (nint)((y + 1) * w) * 4;

            for (int x = 0; x < w; x += 2)
            {
                int uvIdx = ySize + (y / 2) * uvStride + (x / 2);
                int cb = src[uvIdx] - 128;
                int cr = src[uvIdx + uvSize] - 128;

                // Load 4 Y values as int32 lanes: Y0, Y1, Y2, Y3
                var yVec = Vector128.Create(
                    (int)src[yRow0 + x],
                    (int)src[yRow0 + x + 1],
                    (int)src[yRow1 + x],
                    (int)src[yRow1 + x + 1]);

                var cbVec = Vector128.Create(cb);
                var crVec = Vector128.Create(cr);

                // R = Y + ((359 * Cr) >> 8)
                var rVec = Vector128.Add(yVec,
                    Vector128.ShiftRightArithmetic(
                        Vector128.Multiply(crVec, rCoef), 8));

                // G = Y - ((88 * Cb) >> 8) - ((183 * Cr) >> 8)
                var gVec = Vector128.Subtract(
                    Vector128.Subtract(yVec,
                        Vector128.ShiftRightArithmetic(
                            Vector128.Multiply(cbVec, gCoefCb), 8)),
                    Vector128.ShiftRightArithmetic(
                        Vector128.Multiply(crVec, gCoefCr), 8));

                // B = Y + ((454 * Cb) >> 8)
                var bVec = Vector128.Add(yVec,
                    Vector128.ShiftRightArithmetic(
                        Vector128.Multiply(cbVec, bCoef), 8));

                // Branchless clamp to [0, 255]
                rVec = Vector128.Max(zero, Vector128.Min(rVec, maxVal));
                gVec = Vector128.Max(zero, Vector128.Min(gVec, maxVal));
                bVec = Vector128.Max(zero, Vector128.Min(bVec, maxVal));

                // Pack BGRA: (B) | (G<<8) | (R<<16) | (255<<24)
                var pixelVec = bVec
                    | Vector128.ShiftLeft(gVec, 8)
                    | Vector128.ShiftLeft(rVec, 16)
                    | alpha;

                // Store 4 non-contiguous pixels (row0: x, x+1; row1: x, x+1)
                int* pRow0 = (int*)(dstRow0 + x * 4);
                int* pRow1 = (int*)(dstRow1 + x * 4);
                pRow0[0] = pixelVec.GetElement(0);
                pRow0[1] = pixelVec.GetElement(1);
                pRow1[0] = pixelVec.GetElement(2);
                pRow1[1] = pixelVec.GetElement(3);
            }
        }
    }

    // ---------- YUV→RGB conversion ----------

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static void WriteYuvPixel(byte* pDst, int luma, int cb, int cr)
    {
        int r = Clamp(luma + ((YuvRCoef * cr) >> 8));
        int g = Clamp(luma - ((YuvGCoefCb * cb) >> 8) - ((YuvGCoefCr * cr) >> 8));
        int b = Clamp(luma + ((YuvBCoef * cb) >> 8));
        pDst[0] = (byte)b; pDst[1] = (byte)g;
        pDst[2] = (byte)r; pDst[3] = 255;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static int Clamp(int v) => v < 0 ? 0 : v > 255 ? 255 : v;
}
