// YCbCr 4:2:0 planar decoder for .ithmb raw profiles.
using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Intrinsics;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
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

    /// <summary>Scalar fallback for YCbCr 4:2:0.</summary>
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
}
