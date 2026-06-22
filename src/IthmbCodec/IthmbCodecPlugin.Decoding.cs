// Decode algorithms for .ithmb raw profiles.
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
    internal const int YuvRCoef = 359;   //  1.402  (Cr contribution to R)
    internal const int YuvGCoefCb = 88;  // -0.344  (Cb contribution to G)
    internal const int YuvGCoefCr = 183; // -0.714  (Cr contribution to G)
    internal const int YuvBCoef = 454;   //  1.772  (Cb contribution to B)

    // ---------- RGB565 ----------

    /// <summary>Returns false when the input buffer is too small (defensive guard).</summary>
    internal static bool DecodeRgb565(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return false;

        // SIMD path: process 8 pixels per iteration.
        // x64: SSE2 (Sse2.Store/Shuffle). ARM64: NEON (AdvSimd).
        if (Sse2.IsSupported && w >= 8)
            DecodeRgb565_Sse2(src, dst, w, h, littleEndian);
        else if (AdvSimd.IsSupported && w >= 8)
            DecodeRgb565_Neon(src, dst, w, h, littleEndian);
        else
            DecodeRgb565_Scalar(src, dst, w, h, littleEndian);
        return true;
    }

    /// <summary>SSE2-accelerated RGB565→BGRA: 8 pixels (16B→32B) per iteration.</summary>
    /// <remarks>Uses Vector128.StoreUnsafe (movdqu, no alignment required).
    /// The scalar tail (DecodeRgb565_Tail) handles any remaining &lt;8 pixels after the SIMD loop.
    /// Widths smaller than 8 pixels fall through to the scalar fallback path.</remarks>
    private static void DecodeRgb565_Sse2(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pSrcRow = pSrc + (nint)(y * w * 2);
                byte* pDstRow = dst + (nint)(y * w * 4);
                int x = 0;

                    // Main SIMD loop: 8 pixels per iteration
                for (; x + 7 < w; x += 8)
                {
                    Vector128<byte> raw = Vector128.LoadUnsafe(ref *pSrcRow, (nuint)(x * 2));
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

                    Vector128.StoreUnsafe(pxLo, ref *pDstRow, (nuint)(x * 4));
                    Vector128.StoreUnsafe(pxHi, ref *pDstRow, (nuint)((x + 4) * 4));
                }

                // Scalar tail: remaining <8 pixels
                DecodeRgb565_Tail(pSrcRow, pDstRow, x, w, littleEndian);
            }
        }
    }

    /// <summary>ARM64 NEON-accelerated RGB565→BGRA: 8 pixels (16B→32B) per iteration.</summary>
    /// <remarks>Direct transliteration of DecodeRgb565_Sse2 using AdvSimd intrinsics.
    /// Uses ShiftRightLogicalAndNarrowSaturateUnsigned for pack (SQXTUN) and
    /// ZipLow/ZipHigh for interleave (ZIP1/ZIP2).</remarks>
    private static void DecodeRgb565_Neon(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pSrcRow = pSrc + (nint)(y * w * 2);
                byte* pDstRow = dst + (nint)(y * w * 4);
                int x = 0;

                for (; x + 7 < w; x += 8)
                {
                    Vector128<byte> raw = Vector128.LoadUnsafe(ref *pSrcRow, (nuint)(x * 2));
                    Vector128<ushort> v = raw.AsUInt16();

                    // Byte-swap for big-endian profiles
                    if (!littleEndian)
                    {
                        v = AdvSimd.Or(
                            AdvSimd.ShiftRightLogical(v, 8),
                            AdvSimd.ShiftLeftLogical(v, 8)).AsUInt16();
                    }

                    // Extract 5/6/5 bit fields
                    var b5 = AdvSimd.And(v,          Vector128.Create((ushort)0x001F));
                    var g6 = AdvSimd.And(AdvSimd.ShiftRightLogical(v, 5),  Vector128.Create((ushort)0x003F));
                    var r5 = AdvSimd.And(AdvSimd.ShiftRightLogical(v, 11), Vector128.Create((ushort)0x001F));

                    // MSB replicate to 8-bit: (val << k) | (val >> (8-k))
                    var b8 = AdvSimd.Or(AdvSimd.ShiftLeftLogical(b5, 3), AdvSimd.ShiftRightLogical(b5, 2));
                    var g8 = AdvSimd.Or(AdvSimd.ShiftLeftLogical(g6, 2), AdvSimd.ShiftRightLogical(g6, 4));
                    var r8 = AdvSimd.Or(AdvSimd.ShiftLeftLogical(r5, 3), AdvSimd.ShiftRightLogical(r5, 2));

                    // Pack 16-bit → 8-bit via unsigned saturating narrow: SQXTUN
                    // Input is signed short (NEON instruction requires signed source),
                    // output is unsigned byte (values ≤ 255 so saturation is safe).
                    var b64 = AdvSimd.ExtractNarrowingSaturateUnsignedLower(b8.AsInt16()); // Vector64<byte>
                    var g64 = AdvSimd.ExtractNarrowingSaturateUnsignedLower(g8.AsInt16());
                    var r64 = AdvSimd.ExtractNarrowingSaturateUnsignedLower(r8.AsInt16());

                    // Duplicate into full 128-bit: same layout as SSE2 PackUnsignedSaturate(v, v)
                    var bp = Vector128.Create(b64, b64); // [B0..B7, B0..B7]
                    var gp = Vector128.Create(g64, g64); // [G0..G7, G0..G7]
                    var rp = Vector128.Create(r64, r64); // [R0..R7, R0..R7]

                    var alpha = Vector128.Create((byte)255);

                    // Level 1: (B,R) and (G,A) interleaved
                    var br = AdvSimd.Arm64.ZipLow(bp, rp);  // [B0,R0,…,B3,R3, B4,R4,…,B7,R7]
                    var ga = AdvSimd.Arm64.ZipLow(gp, alpha); // [G0,255,…,G3,255, G4,255,…,G7,255]

                    // Level 2: (BR, GA) interleaved → BGRA quads
                    var pxLo = AdvSimd.Arm64.ZipLow(br, ga);  // pixels 0-3: [B0,G0,R0,255, …]
                    var pxHi = AdvSimd.Arm64.ZipHigh(br, ga); // pixels 4-7: [B4,G4,R4,255, …]

                    Vector128.StoreUnsafe(pxLo, ref *pDstRow, (nuint)(x * 4));
                    Vector128.StoreUnsafe(pxHi, ref *pDstRow, (nuint)((x + 4) * 4));
                }

                // Scalar tail: remaining <8 pixels
                DecodeRgb565_Tail(pSrcRow, pDstRow, x, w, littleEndian);
            }
        }
    }

    /// <summary>Scalar fallback for RGB565 decode (pure pointer-based).</summary>
    private static void DecodeRgb565_Scalar(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pDstRow = dst + (nint)(y * w * 4);
                DecodeRgb565_Tail(pSrc + (nint)(y * w * 2), pDstRow, 0, w, littleEndian);
            }
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

    // ---------- RGB555 ----------

    /// <summary>Returns false when the input buffer is too small (defensive guard).</summary>
    internal static bool DecodeRgb555(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return false;

        // SIMD path: process 8 pixels per iteration.
        if (Sse2.IsSupported && w >= 8)
            DecodeRgb555_Sse2(src, dst, w, h, littleEndian);
        else if (AdvSimd.IsSupported && w >= 8)
            DecodeRgb555_Neon(src, dst, w, h, littleEndian);
        else
            DecodeRgb555_Scalar(src, dst, w, h, littleEndian);
        return true;
    }

    /// <summary>SSE2-accelerated RGB555→BGRA: 8 pixels (16B→32B) per iteration.</summary>
    /// <remarks>Uses Vector128.StoreUnsafe (movdqu, no alignment required).
    /// The scalar tail (DecodeRgb555_Tail) handles any remaining &lt;8 pixels after the SIMD loop.
    /// RGB555 bit layout: xRRRRRGGGGGBBBBB (bit 15 unused)
    /// Differences from RGB565:
    ///   - Red:  >> 10 (not >> 11) — skips unused bit 15
    ///   - Green: &amp; 0x001F (5 bits, not 6) — was 0x003F, now same mask as R/B
    ///   - Green MSB-replication: (val &lt;&lt; 3) | (val &gt;&gt; 2) — 5→8 bits (same as R/B)
    /// </remarks>
    private static void DecodeRgb555_Sse2(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pSrcRow = pSrc + (nint)(y * w * 2);
                byte* pDstRow = dst + (nint)(y * w * 4);
                int x = 0;

                for (; x + 7 < w; x += 8)
                {
                    Vector128<byte> raw = Vector128.LoadUnsafe(ref *pSrcRow, (nuint)(x * 2));
                    Vector128<ushort> v = raw.AsUInt16();

                    if (!littleEndian)
                        v = Sse2.Or(Sse2.ShiftRightLogical(v, 8), Sse2.ShiftLeftLogical(v, 8)).AsUInt16();

                    // Extract 5-bit fields: RGB555 = x RRRRR GGGGG BBBBB
                    var r5 = Sse2.And(Sse2.ShiftRightLogical(v, 10), Vector128.Create((ushort)0x001F));
                    var g5 = Sse2.And(Sse2.ShiftRightLogical(v, 5),  Vector128.Create((ushort)0x001F));
                    var b5 = Sse2.And(v,                              Vector128.Create((ushort)0x001F));

                    // MSB replicate to 8-bit: (val << 3) | (val >> 2) — same for all 5-bit fields
                    var r8 = Sse2.Or(Sse2.ShiftLeftLogical(r5, 3), Sse2.ShiftRightLogical(r5, 2));
                    var g8 = Sse2.Or(Sse2.ShiftLeftLogical(g5, 3), Sse2.ShiftRightLogical(g5, 2));
                    var b8 = Sse2.Or(Sse2.ShiftLeftLogical(b5, 3), Sse2.ShiftRightLogical(b5, 2));

                    // Pack to bytes
                    var bp = Sse2.PackUnsignedSaturate(b8.AsInt16(), b8.AsInt16());
                    var gp = Sse2.PackUnsignedSaturate(g8.AsInt16(), g8.AsInt16());
                    var rp = Sse2.PackUnsignedSaturate(r8.AsInt16(), r8.AsInt16());

                    var alpha = Vector128.Create((byte)255);
                    var br = Sse2.UnpackLow(bp, rp);
                    var ga = Sse2.UnpackLow(gp, alpha);
                    var pxLo = Sse2.UnpackLow(br, ga);
                    var pxHi = Sse2.UnpackHigh(br, ga);

                    Vector128.StoreUnsafe(pxLo, ref *pDstRow, (nuint)(x * 4));
                    Vector128.StoreUnsafe(pxHi, ref *pDstRow, (nuint)((x + 4) * 4));
                }

                DecodeRgb555_Tail(pSrcRow, pDstRow, x, w, littleEndian);
            }
        }
    }

    /// <summary>ARM64 NEON-accelerated RGB555→BGRA: 8 pixels (16B→32B) per iteration.</summary>
    /// <remarks>RGB555 bit layout: xRRRRRGGGGGBBBBB (bit 15 unused).
    /// Differences from RGB565 NEON:
    ///   - Red:  >> 10 (not >> 11) — skips unused bit 15
    ///   - Green: &amp; 0x001F (5 bits, not 6)
    ///   - Green MSB-replication: (val &lt;&lt; 3) | (val &gt;&gt; 2) — 5→8 bits (same as R/B)</remarks>
    private static void DecodeRgb555_Neon(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pSrcRow = pSrc + (nint)(y * w * 2);
                byte* pDstRow = dst + (nint)(y * w * 4);
                int x = 0;

                for (; x + 7 < w; x += 8)
                {
                    Vector128<byte> raw = Vector128.LoadUnsafe(ref *pSrcRow, (nuint)(x * 2));
                    Vector128<ushort> v = raw.AsUInt16();

                    if (!littleEndian)
                        v = AdvSimd.Or(
                            AdvSimd.ShiftRightLogical(v, 8),
                            AdvSimd.ShiftLeftLogical(v, 8)).AsUInt16();

                    // Extract 5-bit fields: RGB555 = x RRRRR GGGGG BBBBB
                    var r5 = AdvSimd.And(AdvSimd.ShiftRightLogical(v, 10), Vector128.Create((ushort)0x001F));
                    var g5 = AdvSimd.And(AdvSimd.ShiftRightLogical(v, 5),  Vector128.Create((ushort)0x001F));
                    var b5 = AdvSimd.And(v,                                Vector128.Create((ushort)0x001F));

                    // MSB replicate to 8-bit: all 5-bit fields use (val << 3) | (val >> 2)
                    var r8 = AdvSimd.Or(AdvSimd.ShiftLeftLogical(r5, 3), AdvSimd.ShiftRightLogical(r5, 2));
                    var g8 = AdvSimd.Or(AdvSimd.ShiftLeftLogical(g5, 3), AdvSimd.ShiftRightLogical(g5, 2));
                    var b8 = AdvSimd.Or(AdvSimd.ShiftLeftLogical(b5, 3), AdvSimd.ShiftRightLogical(b5, 2));

                    // Pack 16-bit → 8-bit via unsigned saturating narrow
                    var b64 = AdvSimd.ExtractNarrowingSaturateUnsignedLower(b8.AsInt16());
                    var g64 = AdvSimd.ExtractNarrowingSaturateUnsignedLower(g8.AsInt16());
                    var r64 = AdvSimd.ExtractNarrowingSaturateUnsignedLower(r8.AsInt16());

                    var bp = Vector128.Create(b64, b64);
                    var gp = Vector128.Create(g64, g64);
                    var rp = Vector128.Create(r64, r64);

                    var alpha = Vector128.Create((byte)255);
                    var br = AdvSimd.Arm64.ZipLow(bp, rp);
                    var ga = AdvSimd.Arm64.ZipLow(gp, alpha);
                    var pxLo = AdvSimd.Arm64.ZipLow(br, ga);
                    var pxHi = AdvSimd.Arm64.ZipHigh(br, ga);

                    Vector128.StoreUnsafe(pxLo, ref *pDstRow, (nuint)(x * 4));
                    Vector128.StoreUnsafe(pxHi, ref *pDstRow, (nuint)((x + 4) * 4));
                }

                DecodeRgb555_Tail(pSrcRow, pDstRow, x, w, littleEndian);
            }
        }
    }

    /// <summary>Scalar fallback for RGB555 decode.</summary>
    private static void DecodeRgb555_Scalar(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
                DecodeRgb555_Tail(pSrc + (nint)(y * w * 2),
                    dst + y * w * 4, 0, w, littleEndian);
        }
    }

    /// <summary>Inner row decoder — used by both SIMD (tail) and scalar paths.</summary>
    private static void DecodeRgb555_Tail(byte* pSrc, byte* pDstRow, int xStart, int w, bool littleEndian)
    {
        for (int x = xStart; x < w; x++)
        {
            int idx = x * 2;
            ushort rgb = littleEndian
                ? (ushort)(pSrc[idx] | (pSrc[idx + 1] << 8))
                : (ushort)((pSrc[idx] << 8) | pSrc[idx + 1]);
            int r5 = (rgb >> 10) & 0x1F;  // bits 14-10 (skip unused bit 15)
            int g5 = (rgb >> 5)  & 0x1F;  // bits 9-5
            int b5 = rgb         & 0x1F;  // bits 4-0
            pDstRow[0] = (byte)((b5 << 3) | (b5 >> 2));
            pDstRow[1] = (byte)((g5 << 3) | (g5 >> 2));
            pDstRow[2] = (byte)((r5 << 3) | (r5 >> 2));
            pDstRow[3] = 255;
            pDstRow += 4;
        }
    }

    // ---------- YUV 4:2:2 (UYVY) ----------

    /// <summary>Returns false when the input buffer is too small (defensive guard).</summary>
    internal static bool DecodeYuv422(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return false;
        if ((w & 1) != 0) return false; // pair processing requires even width

        // SIMD: requires SSSE3 (pshufb) on x64 or AdvSimd (VectorTableLookup) on ARM64
        if (Ssse3.IsSupported && (w & 7) == 0)
            DecodeYuv422_SIMD(src, dst, w, h);
        else if (AdvSimd.IsSupported && (w & 7) == 0)
            DecodeYuv422_Neon(src, dst, w, h);
        else
            DecodeYuv422_Scalar(src, dst, w, h);
        return true;
    }

    private static void DecodeYuv422_Scalar(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        for (int y = 0; y < h; y++)
        {
            int rowStart = y * w * 2;
            byte* pDstRow = dst + (nint)(y * w * 4);
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

    /// <summary>SSE2/SSSE3-accelerated UYVY→BGRA (uses pshufb for deinterleave, 32-bit arithmetic).</summary>
    /// <remarks>32-bit integer arithmetic (pmulld, pmaxsd, pminsd) requires SSE4.1 at minimum.
    /// On CPUs with SSSE3 but without SSE4.1, Vector128 intrinsics fall back to software emulation
    /// (correct but slower). The dispatch guard only checks Ssse3.IsSupported for correctness.</remarks>
    private static void DecodeYuv422_SIMD(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        var shufY = Vector128.Create((byte)1, 0x80, 3, 0x80, 5, 0x80, 7, 0x80, 9, 0x80, 11, 0x80, 13, 0x80, 15, 0x80);
        var shufU = Vector128.Create((byte)0, 0x80, 0, 0x80, 4, 0x80, 4, 0x80, 8, 0x80, 8, 0x80, 12, 0x80, 12, 0x80);
        var shufV = Vector128.Create((byte)2, 0x80, 2, 0x80, 6, 0x80, 6, 0x80, 10, 0x80, 10, 0x80, 14, 0x80, 14, 0x80);
        var zeroI = Vector128<int>.Zero;
        var max255 = Vector128.Create(255);
        var alpha = Vector128.Create(255 << 24);
        var rCoef = Vector128.Create(YuvRCoef);
        var gCoefCb = Vector128.Create(YuvGCoefCb);
        var gCoefCr = Vector128.Create(YuvGCoefCr);
        var bCoef = Vector128.Create(YuvBCoef);

        fixed (byte* pSrc = src)
            for (int y = 0; y < h; y++)
                ProcessUyvyRow(pSrc + (nint)(y * w * 2), dst + (nint)(y * w * 4), w,
                    shufY, shufU, shufV, zeroI, max255, alpha,
                    rCoef, gCoefCb, gCoefCr, bCoef);
    }

    /// <summary>Processes one row of UYVY data using SSSE3/SSE2 with 32-bit arithmetic (8 pixels per iteration).</summary>
    /// <remarks>32-bit Vector128&lt;int&gt;.Multiply, Max, Min map to pmulld/pmaxsd/pminsd (SSE4.1+).</remarks>
    private static void ProcessUyvyRow(byte* pSrcRow, byte* pDstRow, int w,
        Vector128<byte> shufY, Vector128<byte> shufU, Vector128<byte> shufV,
        Vector128<int> zeroI, Vector128<int> max255, Vector128<int> alpha,
        Vector128<int> rCoef, Vector128<int> gCoefCb, Vector128<int> gCoefCr, Vector128<int> bCoef)
    {
        for (int x = 0; x < w; x += 8)
        {
            var raw = Vector128.LoadUnsafe(ref *pSrcRow, (nuint)(x * 2));
            var y16 = Ssse3.Shuffle(raw, shufY).AsInt16();
            var u16 = Ssse3.Shuffle(raw, shufU).AsInt16();
            var v16 = Ssse3.Shuffle(raw, shufV).AsInt16();

            // Defer chroma unbias to after 32-bit widening (see below).
            // Applying -128 on 16-bit then zero-extending via UnpackLow loses the sign.

            var zero16 = Vector128<short>.Zero;
            for (int hIdx = 0; hIdx < 2; hIdx++)
            {
                var yI = (hIdx == 0
                    ? Sse2.UnpackLow(y16, zero16)
                    : Sse2.UnpackHigh(y16, zero16)).AsInt32();
                var uI = (hIdx == 0
                    ? Sse2.UnpackLow(u16, zero16)
                    : Sse2.UnpackHigh(u16, zero16)).AsInt32();
                var vI = (hIdx == 0
                    ? Sse2.UnpackLow(v16, zero16)
                    : Sse2.UnpackHigh(v16, zero16)).AsInt32();

                // Unbias chroma: subtract 128 after widening to preserve sign.
                // UnpackLow zero-extends shorts — negative values would become
                // large positive if we subtracted 128 before widening.
                uI = Sse2.Subtract(uI, Vector128.Create(128));
                vI = Sse2.Subtract(vI, Vector128.Create(128));

                var rI = yI + Vector128.ShiftRightArithmetic(vI * rCoef, 8);
                var gI = yI - Vector128.ShiftRightArithmetic(uI * gCoefCb, 8)
                           - Vector128.ShiftRightArithmetic(vI * gCoefCr, 8);
                var bI = yI + Vector128.ShiftRightArithmetic(uI * bCoef, 8);

                rI = Vector128.Max(zeroI, Vector128.Min(rI, max255));
                gI = Vector128.Max(zeroI, Vector128.Min(gI, max255));
                bI = Vector128.Max(zeroI, Vector128.Min(bI, max255));

                var px = bI | Vector128.ShiftLeft(gI, 8)
                    | Vector128.ShiftLeft(rI, 16) | alpha;

                int* pRow = (int*)(pDstRow + x * 4 + hIdx * 16);
                pRow[0] = px.GetElement(0);
                pRow[1] = px.GetElement(1);
                pRow[2] = px.GetElement(2);
                pRow[3] = px.GetElement(3);
            }
        }
    }

    /// <summary>ARM64 NEON-accelerated UYVY→BGRA. Same shuffle masks as SSE2 (VectorTableLookup).</summary>
    private static void DecodeYuv422_Neon(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        // Same shuffle masks as SSE2 — byte indices for VectorTableLookup are identical to pshufb
        var shufY = Vector128.Create((byte)1, 0x80, 3, 0x80, 5, 0x80, 7, 0x80, 9, 0x80, 11, 0x80, 13, 0x80, 15, 0x80);
        var shufU = Vector128.Create((byte)0, 0x80, 0, 0x80, 4, 0x80, 4, 0x80, 8, 0x80, 8, 0x80, 12, 0x80, 12, 0x80);
        var shufV = Vector128.Create((byte)2, 0x80, 2, 0x80, 6, 0x80, 6, 0x80, 10, 0x80, 10, 0x80, 14, 0x80, 14, 0x80);
        var zeroI = Vector128<int>.Zero;
        var max255 = Vector128.Create(255);
        var alpha = Vector128.Create(255 << 24);
        var rCoef = Vector128.Create(YuvRCoef);
        var gCoefCb = Vector128.Create(YuvGCoefCb);
        var gCoefCr = Vector128.Create(YuvGCoefCr);
        var bCoef = Vector128.Create(YuvBCoef);

        fixed (byte* pSrc = src)
            for (int y = 0; y < h; y++)
                ProcessUyvyRow_Neon(pSrc + (nint)(y * w * 2), dst + (nint)(y * w * 4), w,
                    shufY, shufU, shufV, zeroI, max255, alpha,
                    rCoef, gCoefCb, gCoefCr, bCoef);
    }

    /// <summary>Processes one row of UYVY data using NEON AdvSimd with VectorTableLookup (TBL / pshufb equivalent).</summary>
    /// <remarks>Mirrors ProcessUyvyRow but uses AdvSimd intrinsics for the architecture-specific parts.</remarks>
    private static void ProcessUyvyRow_Neon(byte* pSrcRow, byte* pDstRow, int w,
        Vector128<byte> shufY, Vector128<byte> shufU, Vector128<byte> shufV,
        Vector128<int> zeroI, Vector128<int> max255, Vector128<int> alpha,
        Vector128<int> rCoef, Vector128<int> gCoefCb, Vector128<int> gCoefCr, Vector128<int> bCoef)
    {
        for (int x = 0; x < w; x += 8)
        {
            var raw = Vector128.LoadUnsafe(ref *pSrcRow, (nuint)(x * 2));

            // VectorTableLookup (Arm64 overload) = ARM TBL = SSSE3 pshufb — same shuffle masks.
            // The Vector128,Vector128 overload is AdvSimd.Arm64-only (full 128-bit TBL).
            var y16 = AdvSimd.Arm64.VectorTableLookup(raw, shufY).AsInt16();
            var u16 = AdvSimd.Arm64.VectorTableLookup(raw, shufU).AsInt16();
            var v16 = AdvSimd.Arm64.VectorTableLookup(raw, shufV).AsInt16();

            var zero16 = Vector128<short>.Zero;
            for (int hIdx = 0; hIdx < 2; hIdx++)
            {
                // ZipLow/ZipHigh = ARM ZIP1/ZIP2 = SSE2 UnpackLow/UnpackHigh
                var yI = (hIdx == 0
                    ? AdvSimd.Arm64.ZipLow(y16, zero16)
                    : AdvSimd.Arm64.ZipHigh(y16, zero16)).AsInt32();
                var uI = (hIdx == 0
                    ? AdvSimd.Arm64.ZipLow(u16, zero16)
                    : AdvSimd.Arm64.ZipHigh(u16, zero16)).AsInt32();
                var vI = (hIdx == 0
                    ? AdvSimd.Arm64.ZipLow(v16, zero16)
                    : AdvSimd.Arm64.ZipHigh(v16, zero16)).AsInt32();

                // Unbias chroma: subtract 128 after widening
                uI = AdvSimd.Subtract(uI, Vector128.Create(128));
                vI = AdvSimd.Subtract(vI, Vector128.Create(128));

                // BT.601 fixed-point arithmetic (cross-platform Vector128)
                var rI = yI + Vector128.ShiftRightArithmetic(vI * rCoef, 8);
                var gI = yI - Vector128.ShiftRightArithmetic(uI * gCoefCb, 8)
                           - Vector128.ShiftRightArithmetic(vI * gCoefCr, 8);
                var bI = yI + Vector128.ShiftRightArithmetic(uI * bCoef, 8);

                rI = Vector128.Max(zeroI, Vector128.Min(rI, max255));
                gI = Vector128.Max(zeroI, Vector128.Min(gI, max255));
                bI = Vector128.Max(zeroI, Vector128.Min(bI, max255));

                var px = bI | Vector128.ShiftLeft(gI, 8)
                    | Vector128.ShiftLeft(rI, 16) | alpha;

                int* pRow = (int*)(pDstRow + x * 4 + hIdx * 16);
                pRow[0] = px.GetElement(0);
                pRow[1] = px.GetElement(1);
                pRow[2] = px.GetElement(2);
                pRow[3] = px.GetElement(3);
            }
        }
    }

    /// <summary>ARM64 NEON-accelerated interlaced UYVY (F1019).</summary>
    private static void DecodeYuv422Interlaced_Neon(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        int half = ((h + 1) / 2) * w * 2;
        int rowStride = w * 2;

        var shufY = Vector128.Create((byte)1, 0x80, 3, 0x80, 5, 0x80, 7, 0x80, 9, 0x80, 11, 0x80, 13, 0x80, 15, 0x80);
        var shufU = Vector128.Create((byte)0, 0x80, 0, 0x80, 4, 0x80, 4, 0x80, 8, 0x80, 8, 0x80, 12, 0x80, 12, 0x80);
        var shufV = Vector128.Create((byte)2, 0x80, 2, 0x80, 6, 0x80, 6, 0x80, 10, 0x80, 10, 0x80, 14, 0x80, 14, 0x80);
        var zeroI = Vector128<int>.Zero;
        var max255 = Vector128.Create(255);
        var alpha = Vector128.Create(255 << 24);
        var rCoef = Vector128.Create(YuvRCoef);
        var gCoefCb = Vector128.Create(YuvGCoefCb);
        var gCoefCr = Vector128.Create(YuvGCoefCr);
        var bCoef = Vector128.Create(YuvBCoef);

        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                int fieldOffset = (y % 2 == 0) ? 0 : half;
                int rowInField = y / 2;
                int rowStart = fieldOffset + rowInField * rowStride;
                ProcessUyvyRow_Neon(pSrc + rowStart, dst + (nint)(y * w * 4), w,
                    shufY, shufU, shufV, zeroI, max255, alpha,
                    rCoef, gCoefCb, gCoefCr, bCoef);
            }
        }
    }

    /// <summary>Returns false when the input buffer is too small (defensive guard).</summary>
    internal static bool DecodeYuv422Interlaced(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return false;
        if ((w & 1) != 0) return false; // pair processing requires even width

        // SIMD: requires SSSE3 on x64 or AdvSimd on ARM64
        if (Ssse3.IsSupported && (w & 7) == 0)
            DecodeYuv422Interlaced_SIMD(src, dst, w, h);
        else if (AdvSimd.IsSupported && (w & 7) == 0)
            DecodeYuv422Interlaced_Neon(src, dst, w, h);
        else
            DecodeYuv422Interlaced_Scalar(src, dst, w, h);
        return true;
    }

    private static void DecodeYuv422Interlaced_Scalar(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        int half = ((h + 1) / 2) * w * 2;
        int rowStride = w * 2;
        for (int y = 0; y < h; y++)
        {
            int fieldOffset = (y % 2 == 0) ? 0 : half;
            int rowInField = y / 2;
            int rowStart = fieldOffset + rowInField * rowStride;
            byte* pDstRow = dst + (nint)(y * w * 4);
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
        int half = ((h + 1) / 2) * w * 2;
        int rowStride = w * 2;

        var shufY = Vector128.Create((byte)1, 0x80, 3, 0x80, 5, 0x80, 7, 0x80, 9, 0x80, 11, 0x80, 13, 0x80, 15, 0x80);
        var shufU = Vector128.Create((byte)0, 0x80, 0, 0x80, 4, 0x80, 4, 0x80, 8, 0x80, 8, 0x80, 12, 0x80, 12, 0x80);
        var shufV = Vector128.Create((byte)2, 0x80, 2, 0x80, 6, 0x80, 6, 0x80, 10, 0x80, 10, 0x80, 14, 0x80, 14, 0x80);
        var zeroI = Vector128<int>.Zero;
        var max255 = Vector128.Create(255);
        var alpha = Vector128.Create(255 << 24);
        var rCoef = Vector128.Create(YuvRCoef);
        var gCoefCb = Vector128.Create(YuvGCoefCb);
        var gCoefCr = Vector128.Create(YuvGCoefCr);
        var bCoef = Vector128.Create(YuvBCoef);

        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                int fieldOffset = (y % 2 == 0) ? 0 : half;
                int rowInField = y / 2;
                int rowStart = fieldOffset + rowInField * rowStride;
                ProcessUyvyRow(pSrc + rowStart, dst + (nint)(y * w * 4), w,
                    shufY, shufU, shufV, zeroI, max255, alpha,
                    rCoef, gCoefCb, gCoefCr, bCoef);
            }
        }
    }

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
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return false;
        if ((w & 1) != 0) return false; // pair processing requires even width

        for (int y = 0; y < h; y++)
        {
            int rowStart = y * w * 2;
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
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return false;

        for (int y = 0; y < h; y++)
        {
            int rowStart = y * w * 2;
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
        long totalPixels = (long)w * h;
        int ySize = (int)totalPixels; // ≤ 25M due to 50 MB file limit
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
