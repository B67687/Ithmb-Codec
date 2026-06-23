// Decode algorithms for .ithmb raw profiles — RGB565 and RGB555.
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
        if (w <= 0 || h <= 0) return false;
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
    internal static bool DecodeRgb555(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian, bool swapRgbChannels = false)
    {
        if (w <= 0 || h <= 0) return false;
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return false;

        // SIMD path: process 8 pixels per iteration.
        if (Sse2.IsSupported && w >= 8)
            DecodeRgb555_Sse2(src, dst, w, h, littleEndian, swapRgbChannels);
        else if (AdvSimd.IsSupported && w >= 8)
            DecodeRgb555_Neon(src, dst, w, h, littleEndian, swapRgbChannels);
        else
            DecodeRgb555_Scalar(src, dst, w, h, littleEndian, swapRgbChannels);
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
    private static void DecodeRgb555_Sse2(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian, bool swapRgbChannels)
    {
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pSrcRow = pSrc + (nint)(y * w * 2);
                byte* pDstRow = dst + (nint)(y * w * 4);
                int x = 0;

                if (swapRgbChannels)
                {
                    // BGR;15 layout: x BBBBB GGGGG RRRRR
                    for (; x + 7 < w; x += 8)
                    {
                        Vector128<byte> raw = Vector128.LoadUnsafe(ref *pSrcRow, (nuint)(x * 2));
                        Vector128<ushort> v = raw.AsUInt16();

                        if (!littleEndian)
                            v = Sse2.Or(Sse2.ShiftRightLogical(v, 8), Sse2.ShiftLeftLogical(v, 8)).AsUInt16();

                        // BGR;15: x BBBBB GGGGG RRRRR — swap R↔B extraction
                        var b5 = Sse2.And(Sse2.ShiftRightLogical(v, 10), Vector128.Create((ushort)0x001F));
                        var g5 = Sse2.And(Sse2.ShiftRightLogical(v, 5),  Vector128.Create((ushort)0x001F));
                        var r5 = Sse2.And(v,                              Vector128.Create((ushort)0x001F));

                        var r8 = Sse2.Or(Sse2.ShiftLeftLogical(r5, 3), Sse2.ShiftRightLogical(r5, 2));
                        var g8 = Sse2.Or(Sse2.ShiftLeftLogical(g5, 3), Sse2.ShiftRightLogical(g5, 2));
                        var b8 = Sse2.Or(Sse2.ShiftLeftLogical(b5, 3), Sse2.ShiftRightLogical(b5, 2));

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
                }
                else
                {
                    // Standard RGB555: x RRRRR GGGGG BBBBB
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
                }

                DecodeRgb555_Tail(pSrcRow, pDstRow, x, w, littleEndian, swapRgbChannels);
            }
        }
    }

    /// <summary>ARM64 NEON-accelerated RGB555→BGRA: 8 pixels (16B→32B) per iteration.</summary>
    /// <remarks>RGB555 bit layout: xRRRRRGGGGGBBBBB (bit 15 unused).
    /// Differences from RGB565 NEON:
    ///   - Red:  >> 10 (not >> 11) — skips unused bit 15
    ///   - Green: &amp; 0x001F (5 bits, not 6)
    ///   - Green MSB-replication: (val &lt;&lt; 3) | (val &gt;&gt; 2) — 5→8 bits (same as R/B)</remarks>
    private static void DecodeRgb555_Neon(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian, bool swapRgbChannels)
    {
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pSrcRow = pSrc + (nint)(y * w * 2);
                byte* pDstRow = dst + (nint)(y * w * 4);
                int x = 0;

                if (swapRgbChannels)
                {
                    // BGR;15 layout: x BBBBB GGGGG RRRRR
                    for (; x + 7 < w; x += 8)
                    {
                        Vector128<byte> raw = Vector128.LoadUnsafe(ref *pSrcRow, (nuint)(x * 2));
                        Vector128<ushort> v = raw.AsUInt16();

                        if (!littleEndian)
                            v = AdvSimd.Or(
                                AdvSimd.ShiftRightLogical(v, 8),
                                AdvSimd.ShiftLeftLogical(v, 8)).AsUInt16();

                        // BGR;15: x BBBBB GGGGG RRRRR — swap R↔B extraction
                        var b5 = AdvSimd.And(AdvSimd.ShiftRightLogical(v, 10), Vector128.Create((ushort)0x001F));
                        var g5 = AdvSimd.And(AdvSimd.ShiftRightLogical(v, 5),  Vector128.Create((ushort)0x001F));
                        var r5 = AdvSimd.And(v,                                Vector128.Create((ushort)0x001F));

                        var r8 = AdvSimd.Or(AdvSimd.ShiftLeftLogical(r5, 3), AdvSimd.ShiftRightLogical(r5, 2));
                        var g8 = AdvSimd.Or(AdvSimd.ShiftLeftLogical(g5, 3), AdvSimd.ShiftRightLogical(g5, 2));
                        var b8 = AdvSimd.Or(AdvSimd.ShiftLeftLogical(b5, 3), AdvSimd.ShiftRightLogical(b5, 2));

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
                }
                else
                {
                    // Standard RGB555: x RRRRR GGGGG BBBBB
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
                }

                DecodeRgb555_Tail(pSrcRow, pDstRow, x, w, littleEndian, swapRgbChannels);
            }
        }
    }

    /// <summary>Scalar fallback for RGB555 decode.</summary>
    private static void DecodeRgb555_Scalar(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian, bool swapRgbChannels)
    {
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
                DecodeRgb555_Tail(pSrc + (nint)(y * w * 2),
                    dst + y * w * 4, 0, w, littleEndian, swapRgbChannels);
        }
    }

    /// <summary>Inner row decoder — used by both SIMD (tail) and scalar paths.</summary>
    private static void DecodeRgb555_Tail(byte* pSrc, byte* pDstRow, int xStart, int w, bool littleEndian, bool swapRgbChannels)
    {
        for (int x = xStart; x < w; x++)
        {
            int idx = x * 2;
            ushort rgb = littleEndian
                ? (ushort)(pSrc[idx] | (pSrc[idx + 1] << 8))
                : (ushort)((pSrc[idx] << 8) | pSrc[idx + 1]);
            int r5, g5, b5;
            if (swapRgbChannels)
            {
                // BGR;15: x BBBBB GGGGG RRRRR
                b5 = (rgb >> 10) & 0x1F;
                g5 = (rgb >> 5)  & 0x1F;
                r5 = rgb         & 0x1F;
            }
            else
            {
                // Standard RGB555: x RRRRR GGGGG BBBBB
                r5 = (rgb >> 10) & 0x1F;
                g5 = (rgb >> 5)  & 0x1F;
                b5 = rgb         & 0x1F;
            }
            pDstRow[0] = (byte)((b5 << 3) | (b5 >> 2));
            pDstRow[1] = (byte)((g5 << 3) | (g5 >> 2));
            pDstRow[2] = (byte)((r5 << 3) | (r5 >> 2));
            pDstRow[3] = 255;
            pDstRow += 4;
        }
    }
}
