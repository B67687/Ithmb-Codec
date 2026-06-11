// Licensed under MIT. See LICENSE in the repository root.
// 
// Decode algorithms for .ithmb raw profiles.
// Separated from plugin ABI glue for independent AOT compilation.
using System;
using System.Runtime.CompilerServices;
using System.Runtime.Intrinsics;
using System.Runtime.Intrinsics.X86;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // BT.601-7 (SDTV) Y′CbCr → R′G′B′ coefficients, 8:8 fixed-point
    private const int YuvRCoef = 359;   //  1.402  (Cr contribution to R)
    private const int YuvGCoefCb = 88;  // -0.344  (Cb contribution to G)
    private const int YuvGCoefCr = 183; // -0.714  (Cr contribution to G)
    private const int YuvBCoef = 454;   //  1.772  (Cb contribution to B)

    // ---------- RGB565 ----------

    internal static void DecodeRgb565(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return;

        // SIMD path: process 8 pixels per iteration on x64 (SSE2)
        if (Sse2.IsSupported && w >= 8)
        {
            DecodeRgb565_Sse2(src, dst, w, h, littleEndian);
            return;
        }

        // Scalar fallback
        DecodeRgb565_Scalar(src, dst, w, h, littleEndian);
    }

    /// <summary>SSE2-accelerated RGB565→BGRA: 8 pixels (16B→32B) per iteration.</summary>
    private static void DecodeRgb565_Sse2(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pSrcRow = pSrc + y * w * 2;
                byte* pDstRow = dst + y * w * 4;
                int x = 0;

                // Main SIMD loop: 8 pixels per iteration
                for (; x + 7 < w; x += 8)
                {
                    Vector128<byte> raw = Sse2.LoadVector128(pSrcRow + x * 2);
                    Vector128<ushort> v = raw.AsUInt16();

                    // Byte-swap for big-endian profiles
                    if (!littleEndian)
                    {
                        v = Sse2.Or(
                            Sse2.ShiftRightLogical(v, 8),
                            Sse2.ShiftLeftLogical(v, 8)).AsUInt16();
                    }

                    // Extract 5/6/5 bit fields
                    var b5 = Sse2.And(v,          Vector128.Create((ushort)0x001F));
                    var g6 = Sse2.And(Sse2.ShiftRightLogical(v, 5),  Vector128.Create((ushort)0x003F));
                    var r5 = Sse2.And(Sse2.ShiftRightLogical(v, 11), Vector128.Create((ushort)0x001F));

                    // MSB replicate to 8-bit: (val << k) | (val >> (8-k))
                    var b8 = Sse2.Or(Sse2.ShiftLeftLogical(b5, 3), Sse2.ShiftRightLogical(b5, 2));
                    var g8 = Sse2.Or(Sse2.ShiftLeftLogical(g6, 2), Sse2.ShiftRightLogical(g6, 4));
                    var r8 = Sse2.Or(Sse2.ShiftLeftLogical(r5, 3), Sse2.ShiftRightLogical(r5, 2));

                    // Pack to bytes: 16-bit → 8-bit (saturating, safe since values ≤ 255)
                    var bp = Sse2.PackUnsignedSaturate(b8.AsInt16(), b8.AsInt16()); // [B0..B7, B0..B7]
                    var gp = Sse2.PackUnsignedSaturate(g8.AsInt16(), g8.AsInt16()); // [G0..G7, G0..G7]
                    var rp = Sse2.PackUnsignedSaturate(r8.AsInt16(), r8.AsInt16()); // [R0..R7, R0..R7]

                    var alpha = Vector128.Create((byte)255);

                    // Level 1: (B,R) and (G,A) interleaved
                    var br = Sse2.UnpackLow(bp, rp);    // [B0,R0,…,B3,R3, B4,R4,…,B7,R7]
                    var ga = Sse2.UnpackLow(gp, alpha); // [G0,255,…,G3,255, G4,255,…,G7,255]

                    // Level 2: (BR, GA) interleaved → BGRA quads
                    var pxLo = Sse2.UnpackLow(br, ga);  // pixels 0-3: [B0,G0,R0,255, B1,G1,R1,255, B2,G2,R2,255, B3,G3,R3,255]
                    var pxHi = Sse2.UnpackHigh(br, ga); // pixels 4-7: [B4,G4,R4,255, B5,G5,R5,255, B6,G6,R6,255, B7,G7,R7,255]

                    Sse2.Store(pDstRow + x * 4, pxLo);
                    Sse2.Store(pDstRow + (x + 4) * 4, pxHi);
                }

                // Scalar tail: remaining <8 pixels
                DecodeRgb565_Tail(pSrcRow, pDstRow, x, w, littleEndian);
            }
        }
    }

    /// <summary>Scalar fallback for RGB565 decode (pure pointer-based).</summary>
    private static void DecodeRgb565_Scalar(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        for (int y = 0; y < h; y++)
        {
            byte* pDstRow = dst + y * w * 4;
            DecodeRgb565_Tail((byte*)Unsafe.AsPointer(ref System.Runtime.InteropServices.MemoryMarshal.GetReference(src)) + y * w * 2, pDstRow, 0, w, littleEndian);
        }
    }

    /// <summary>Inner row decoder — processes remaining pixels from xStart to w.</summary>
    /// <remarks>Takes byte* pSrc for SIMD path (tail after SSE2). The scalar decoder wraps
    /// this by fixing up the span pointer via a GCHandle internally.</remarks>
    private static void DecodeRgb565_Tail(byte* pSrc, byte* pDstRow, int xStart, int w, bool littleEndian)
    {
        for (int x = xStart; x < w; x++)
        {
            int idx = x * 2;
            ushort rgb = littleEndian
                ? (ushort)(pSrc[idx] | (pSrc[idx + 1] << 8))
                : (ushort)((pSrc[idx] << 8) | pSrc[idx + 1]);
            int r5 = (rgb >> 11) & 0x1F;
            int g6 = (rgb >> 5) & 0x3F;
            int b5 = rgb & 0x1F;
            pDstRow[0] = (byte)((b5 << 3) | (b5 >> 2));
            pDstRow[1] = (byte)((g6 << 2) | (g6 >> 4));
            pDstRow[2] = (byte)((r5 << 3) | (r5 >> 2));
            pDstRow[3] = 255;
            pDstRow += 4;
        }
    }

    // ---------- YUV 4:2:2 (UYVY) ----------

    internal static void DecodeYuv422(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return;

        // SIMD: requires SSSE3 (pshufb for UYVY deinterleave) and w divisible by 8
        if (Ssse3.IsSupported && (w & 7) == 0)
            DecodeYuv422_SIMD(src, dst, w, h);
        else
            DecodeYuv422_Scalar(src, dst, w, h);
    }

    private static void DecodeYuv422_Scalar(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        for (int y = 0; y < h; y++)
        {
            int rowStart = y * w * 2;
            byte* pDstRow = dst + y * w * 4;
            for (int x = 0; x < w; x += 2)
            {
                int idx = rowStart + x * 2;
                int u = src[idx] - 128;
                int y0 = src[idx + 1];
                int v = src[idx + 2] - 128;
                int y1 = src[idx + 3];

                WriteYuvPixel(pDstRow, y0, u, v);
                if (x + 1 < w) WriteYuvPixel(pDstRow + 4, y1, u, v);
                pDstRow += 8;
            }
        }
    }

    /// <summary>SSE2/SSSE3-accelerated UYVY→BGRA (requires SSSE3 for pshufb deinterleave).</summary>
    private static void DecodeYuv422_SIMD(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        var shufY = Vector128.Create((byte)1, 3, 5, 7, 9, 11, 13, 15, 0, 0, 0, 0, 0, 0, 0, 0);
        var shufU = Vector128.Create((byte)0, 0, 4, 4, 8, 8, 12, 12, 0, 0, 0, 0, 0, 0, 0, 0);
        var shufV = Vector128.Create((byte)2, 2, 6, 6, 10, 10, 14, 14, 0, 0, 0, 0, 0, 0, 0, 0);

        var zero = Vector128.Create((short)0);
        var vec128 = Vector128.Create((short)128);
        var rCoef = Vector128.Create((short)YuvRCoef);
        var gCoefCb = Vector128.Create((short)YuvGCoefCb);
        var gCoefCr = Vector128.Create((short)YuvGCoefCr);
        var bCoef = Vector128.Create((short)YuvBCoef);

        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                int rowStart = y * w * 2;
                byte* pSrcRow = pSrc + rowStart;
                byte* pDstRow = dst + y * w * 4;

                for (int x = 0; x < w; x += 8)
                {
                    var raw = Sse2.LoadVector128(pSrcRow + x * 2);
                    var sY = Ssse3.Shuffle(raw, shufY).AsInt16();
                    var sU = Ssse3.Shuffle(raw, shufU).AsInt16();
                    var sV = Ssse3.Shuffle(raw, shufV).AsInt16();

                // Unbias chroma
                sU = Sse2.Subtract(sU, vec128);
                sV = Sse2.Subtract(sV, vec128);

                // R = Y + ((359 * V) >> 8)
                var rV = Sse2.Add(sY,
                    Sse2.ShiftRightArithmetic(Sse2.MultiplyHigh(sV, rCoef), 8));
                // G = Y - ((88 * U) >> 8) - ((183 * V) >> 8)
                var gV = Sse2.Subtract(
                    Sse2.Subtract(sY,
                        Sse2.ShiftRightArithmetic(Sse2.MultiplyHigh(sU, gCoefCb), 8)),
                    Sse2.ShiftRightArithmetic(Sse2.MultiplyHigh(sV, gCoefCr), 8));
                // B = Y + ((454 * U) >> 8)
                var bV = Sse2.Add(sY,
                    Sse2.ShiftRightArithmetic(Sse2.MultiplyHigh(sU, bCoef), 8));

                // Clamp
                var max255 = Vector128.Create((short)255);
                rV = Sse2.Max(zero, Sse2.Min(rV, max255));
                gV = Sse2.Max(zero, Sse2.Min(gV, max255));
                bV = Sse2.Max(zero, Sse2.Min(bV, max255));

                // Pack to bytes + interleave BGRA
                var bp = Sse2.PackUnsignedSaturate(bV, bV);
                var gp = Sse2.PackUnsignedSaturate(gV, gV);
                var rp = Sse2.PackUnsignedSaturate(rV, rV);

                var alpha = Vector128.Create((byte)255);
                var br = Sse2.UnpackLow(bp, rp);
                var ga = Sse2.UnpackLow(gp, alpha);
                var pxLo = Sse2.UnpackLow(br, ga);
                var pxHi = Sse2.UnpackHigh(br, ga);

                Sse2.Store(pDstRow + x * 4, pxLo);
                Sse2.Store(pDstRow + (x + 4) * 4, pxHi);
            }
        }
        } // close fixed block
    }

    internal static void DecodeYuv422Interlaced(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return;

        // SIMD: requires SSSE3 (pshufb for UYVY deinterleave) and w divisible by 8
        if (Ssse3.IsSupported && (w & 7) == 0)
            DecodeYuv422Interlaced_SIMD(src, dst, w, h);
        else
            DecodeYuv422Interlaced_Scalar(src, dst, w, h);
    }

    private static void DecodeYuv422Interlaced_Scalar(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        int half = (h / 2) * w * 2;
        int rowStride = w * 2;
        for (int y = 0; y < h; y++)
        {
            int fieldOffset = (y % 2 == 0) ? 0 : half;
            int rowInField = y / 2;
            int rowStart = fieldOffset + rowInField * rowStride;
            byte* pDstRow = dst + y * w * 4;
            for (int x = 0; x < w; x += 2)
            {
                int idx = rowStart + x * 2;
                int u = src[idx] - 128;
                int y0 = src[idx + 1];
                int v = src[idx + 2] - 128;
                int y1 = src[idx + 3];
                WriteYuvPixel(pDstRow, y0, u, v);
                if (x + 1 < w) WriteYuvPixel(pDstRow + 4, y1, u, v);
                pDstRow += 8;
            }
        }
    }

    /// <summary>SIMD-accelerated interlaced UYVY (F1019). Inner loop identical to non-interlaced.</summary>
    private static void DecodeYuv422Interlaced_SIMD(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        int half = (h / 2) * w * 2;
        int rowStride = w * 2;

        var shufY = Vector128.Create((byte)1, 3, 5, 7, 9, 11, 13, 15, 0, 0, 0, 0, 0, 0, 0, 0);
        var shufU = Vector128.Create((byte)0, 0, 4, 4, 8, 8, 12, 12, 0, 0, 0, 0, 0, 0, 0, 0);
        var shufV = Vector128.Create((byte)2, 2, 6, 6, 10, 10, 14, 14, 0, 0, 0, 0, 0, 0, 0, 0);

        var zero = Vector128<short>.Zero;
        var vec128 = Vector128.Create((short)128);
        var rCoef = Vector128.Create((short)YuvRCoef);
        var gCoefCb = Vector128.Create((short)YuvGCoefCb);
        var gCoefCr = Vector128.Create((short)YuvGCoefCr);
        var bCoef = Vector128.Create((short)YuvBCoef);

        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                int fieldOffset = (y % 2 == 0) ? 0 : half;
                int rowInField = y / 2;
                int rowStart = fieldOffset + rowInField * rowStride;
                byte* pSrcRow = pSrc + rowStart;
                byte* pDstRow = dst + y * w * 4;

                for (int x = 0; x < w; x += 8)
                {
                    var raw = Sse2.LoadVector128(pSrcRow + x * 2);
                    var sY = Ssse3.Shuffle(raw, shufY).AsInt16();
                var sU = Ssse3.Shuffle(raw, shufU).AsInt16();
                var sV = Ssse3.Shuffle(raw, shufV).AsInt16();

                sU = Sse2.Subtract(sU, vec128);
                sV = Sse2.Subtract(sV, vec128);

                var rV = Sse2.Add(sY,
                    Sse2.ShiftRightArithmetic(Sse2.MultiplyHigh(sV, rCoef), 8));
                var gV = Sse2.Subtract(
                    Sse2.Subtract(sY,
                        Sse2.ShiftRightArithmetic(Sse2.MultiplyHigh(sU, gCoefCb), 8)),
                    Sse2.ShiftRightArithmetic(Sse2.MultiplyHigh(sV, gCoefCr), 8));
                var bV = Sse2.Add(sY,
                    Sse2.ShiftRightArithmetic(Sse2.MultiplyHigh(sU, bCoef), 8));

                var max255 = Vector128.Create((short)255);
                rV = Sse2.Max(zero, Sse2.Min(rV, max255));
                gV = Sse2.Max(zero, Sse2.Min(gV, max255));
                bV = Sse2.Max(zero, Sse2.Min(bV, max255));

                var bp = Sse2.PackUnsignedSaturate(bV, bV);
                var gp = Sse2.PackUnsignedSaturate(gV, gV);
                var rp = Sse2.PackUnsignedSaturate(rV, rV);

                var alpha = Vector128.Create((byte)255);
                var br = Sse2.UnpackLow(bp, rp);
                var ga = Sse2.UnpackLow(gp, alpha);
                var pxLo = Sse2.UnpackLow(br, ga);
                var pxHi = Sse2.UnpackHigh(br, ga);

                Sse2.Store(pDstRow + x * 4, pxLo);
                Sse2.Store(pDstRow + (x + 4) * 4, pxHi);
            }
        }
        } // close fixed block
    }

    // ---------- YCbCr 4:2:0 (planar) ----------

    internal static void DecodeYcbcr420(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        int ySize = w * h;
        int uvSize = ((w + 1) / 2) * ((h + 1) / 2);
        long expectedBytes = (long)ySize + uvSize * 2;
        if (src.Length < expectedBytes) return;

        // SIMD path: requires even dimensions (2×2 blocks fit perfectly in Vector128<int>)
        if (Vector128.IsHardwareAccelerated && (w & 1) == 0 && (h & 1) == 0)
            DecodeYcbcr420_SIMD(src, dst, w, h, ySize, uvSize);
        else
            DecodeYcbcr420_Scalar(src, dst, w, h, ySize, uvSize);
    }

    /// <summary>Scalar fallback for YCbCr 4:2:0 (extracted from original body).</summary>
    private static void DecodeYcbcr420_Scalar(ReadOnlySpan<byte> src, byte* dst,
        int w, int h, int ySize, int uvSize)
    {
        int uvStride = w / 2;
        for (int y = 0; y < h; y += 2)
        {
            for (int x = 0; x < w; x += 2)
            {
                int uvIdx = ySize + (y / 2) * uvStride + (x / 2);
                int cb = src[uvIdx] - 128;
                int cr = src[uvIdx + uvSize] - 128;

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
        int uvStride = w / 2;

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
            byte* dstRow0 = dst + y * w * 4;
            byte* dstRow1 = dst + (y + 1) * w * 4;

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
