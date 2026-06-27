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
    // ---------- RGB565 ----------

    // ---------- RGB565 ----------

    /// <summary>Returns false when the input buffer is too small (defensive guard).</summary>
    internal static bool DecodeRgb565(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian, bool swapRgbChannels = false)
    {
        if (w <= 0 || h <= 0) return false;
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return false;

        // SIMD path: process 8 pixels per iteration.
        // x64: SSE2 (Sse2.Store/Shuffle). ARM64: NEON (AdvSimd).
        if (Sse2.IsSupported && w >= 8)
            DecodeRgb565_Sse2(src, dst, w, h, littleEndian, swapRgbChannels);
        else if (AdvSimd.IsSupported && w >= 8)
            DecodeRgb565_Neon(src, dst, w, h, littleEndian, swapRgbChannels);
        else
            DecodeRgb565_Scalar(src, dst, w, h, littleEndian, swapRgbChannels);
        return true;
    }

    /// <summary>Parameterized SSE2 SIMD loop shared by RGB565 and RGB555 decoders.</summary>
#pragma warning disable CA1857 // constant preferred — values ARE const per call site; AggressiveInlining lets JIT propagate
    private static void DecodeRgbX_Sse2(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool le, bool swapRgbChannels,
        int rShift, int gShift, int bShift, int gMask, int gMsbShift,
        delegate* managed<byte*, byte*, int, int, bool, bool, void> tail)
    {
        int gRightShift = 8 - 2 * gMsbShift;
        int rowStride = (int)((long)src.Length / h);
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pSrcRow = pSrc + (nint)(y * rowStride);
                byte* pDstRow = dst + (nint)(y * w * 4);
                int x = 0;

                // Single SIMD loop: extraction shifts conditional on swapRgbChannels
                for (; x + 7 < w; x += 8)
                {
                    Vector128<byte> raw = Vector128.LoadUnsafe(ref *pSrcRow, (nuint)(x * 2));
                    Vector128<ushort> v = raw.AsUInt16();

                    if (!le)
                        v = Sse2.Or(
                            Sse2.ShiftRightLogical(v, 8),
                            Sse2.ShiftLeftLogical(v, 8)).AsUInt16();

                    // Extract fields: bShift/rShift swap when swapRgbChannels
                    var bF = Sse2.And(Sse2.ShiftRightLogical(v, (byte)(swapRgbChannels ? rShift : bShift)), Vector128.Create((ushort)0x001F));
                    var gF = Sse2.And(Sse2.ShiftRightLogical(v, (byte)gShift), Vector128.Create((ushort)gMask));
                    var rF = Sse2.And(Sse2.ShiftRightLogical(v, (byte)(swapRgbChannels ? bShift : rShift)), Vector128.Create((ushort)0x001F));

                    // MSB replicate to 8-bit
                    var b8 = Sse2.Or(Sse2.ShiftLeftLogical(bF, 3), Sse2.ShiftRightLogical(bF, 2));
                    var g8 = Sse2.Or(Sse2.ShiftLeftLogical(gF, (byte)gMsbShift), Sse2.ShiftRightLogical(gF, (byte)gRightShift));
                    var r8 = Sse2.Or(Sse2.ShiftLeftLogical(rF, 3), Sse2.ShiftRightLogical(rF, 2));

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

                tail(pSrcRow, pDstRow, x, w, le, swapRgbChannels);
            }
        }
    }
#pragma warning restore CA1857

    /// <summary>SSE2-accelerated RGB565→BGRA: 8 pixels (16B→32B) per iteration.</summary>
    /// <remarks>Uses Vector128.StoreUnsafe (movdqu, no alignment required).
    /// The scalar tail (DecodeRgb565_Tail) handles any remaining &lt;8 pixels after the SIMD loop.
    /// Widths smaller than 8 pixels fall through to the scalar fallback path.</remarks>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static void DecodeRgb565_Sse2(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian, bool swapRgbChannels)
        => DecodeRgbX_Sse2(src, dst, w, h, littleEndian, swapRgbChannels, 11, 5, 0, 0x003F, 2, &DecodeRgb565_Tail);

    /// <summary>Parameterized ARM64 NEON SIMD loop shared by RGB565 and RGB555 decoders.</summary>
#pragma warning disable CA1857
    private static void DecodeRgbX_Neon(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool le, bool swapRgbChannels,
        int rShift, int gShift, int bShift, int gMask, int gMsbShift,
        delegate* managed<byte*, byte*, int, int, bool, bool, void> tail)
    {
        int gRightShift = 8 - 2 * gMsbShift;
        int rowStride = (int)((long)src.Length / h);
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pSrcRow = pSrc + (nint)(y * rowStride);
                byte* pDstRow = dst + (nint)(y * w * 4);
                int x = 0;

                // Single SIMD loop: extraction shifts conditional on swapRgbChannels
                for (; x + 7 < w; x += 8)
                {
                    Vector128<byte> raw = Vector128.LoadUnsafe(ref *pSrcRow, (nuint)(x * 2));
                    Vector128<ushort> v = raw.AsUInt16();

                    if (!le)
                        v = AdvSimd.Or(
                            AdvSimd.ShiftRightLogical(v, 8),
                            AdvSimd.ShiftLeftLogical(v, 8)).AsUInt16();

                    // Extract fields: bShift/rShift swap when swapRgbChannels
                    var bF = AdvSimd.And(AdvSimd.ShiftRightLogical(v, (byte)(swapRgbChannels ? rShift : bShift)), Vector128.Create((ushort)0x001F));
                    var gF = AdvSimd.And(AdvSimd.ShiftRightLogical(v, (byte)gShift), Vector128.Create((ushort)gMask));
                    var rF = AdvSimd.And(AdvSimd.ShiftRightLogical(v, (byte)(swapRgbChannels ? bShift : rShift)), Vector128.Create((ushort)0x001F));

                    // MSB replicate to 8-bit
                    var b8 = AdvSimd.Or(AdvSimd.ShiftLeftLogical(bF, 3), AdvSimd.ShiftRightLogical(bF, 2));
                    var g8 = AdvSimd.Or(AdvSimd.ShiftLeftLogical(gF, (byte)gMsbShift), AdvSimd.ShiftRightLogical(gF, (byte)gRightShift));
                    var r8 = AdvSimd.Or(AdvSimd.ShiftLeftLogical(rF, 3), AdvSimd.ShiftRightLogical(rF, 2));

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

                tail(pSrcRow, pDstRow, x, w, le, swapRgbChannels);
            }
        }
    }
#pragma warning restore CA1857

    /// <summary>ARM64 NEON-accelerated RGB565→BGRA: 8 pixels (16B→32B) per iteration.</summary>
    /// <remarks>Direct transliteration of DecodeRgb565_Sse2 using AdvSimd intrinsics.
    /// Uses ShiftRightLogicalAndNarrowSaturateUnsigned for pack (SQXTUN) and
    /// ZipLow/ZipHigh for interleave (ZIP1/ZIP2).</remarks>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static void DecodeRgb565_Neon(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian, bool swapRgbChannels)
        => DecodeRgbX_Neon(src, dst, w, h, littleEndian, swapRgbChannels, 11, 5, 0, 0x003F, 2, &DecodeRgb565_Tail);

    /// <summary>Scalar fallback for RGB565 decode (pure pointer-based).</summary>
    private static void DecodeRgb565_Scalar(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian, bool swapRgbChannels = false)
    {
        int rowStride = (int)((long)src.Length / h);
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pDstRow = dst + (nint)(y * w * 4);
                DecodeRgb565_Tail(pSrc + (nint)(y * rowStride), pDstRow, 0, w, littleEndian, swapRgbChannels);
            }
        }
    }

    /// <summary>Inner row decoder — processes remaining pixels from xStart to w.</summary>
    /// <remarks>Takes byte* pSrc for SIMD path (tail after SSE2). The scalar decoder wraps
    /// this by fixing up the span pointer via a GCHandle internally.</remarks>
    private static void DecodeRgb565_Tail(byte* pSrc, byte* pDstRow, int xStart, int w, bool littleEndian, bool swapRgbChannels = false)
    {
        byte* pDst = pDstRow + xStart * 4;
        for (int x = xStart; x < w; x++)
        {
            int idx = x * 2;
            ushort rgb = littleEndian
                ? (ushort)(pSrc[idx] | (pSrc[idx + 1] << 8))
                : (ushort)((pSrc[idx] << 8) | pSrc[idx + 1]);
            int r5 = (rgb >> 11) & 0x1F;
            int g6 = (rgb >> 5) & 0x3F;
            int b5 = rgb & 0x1F;
            if (swapRgbChannels)
            {
                // BGR565: output B->R slot, R->B slot
                pDst[0] = (byte)((r5 << 3) | (r5 >> 2));
                pDst[1] = (byte)((g6 << 2) | (g6 >> 4));
                pDst[2] = (byte)((b5 << 3) | (b5 >> 2));
            }
            else
            {
                pDst[0] = (byte)((b5 << 3) | (b5 >> 2));
                pDst[1] = (byte)((g6 << 2) | (g6 >> 4));
                pDst[2] = (byte)((r5 << 3) | (r5 >> 2));
            }
            pDst[3] = 255;
            pDst += 4;
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
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static void DecodeRgb555_Sse2(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian, bool swapRgbChannels)
        => DecodeRgbX_Sse2(src, dst, w, h, littleEndian, swapRgbChannels, 10, 5, 0, 0x001F, 3, &DecodeRgb555_Tail);

    /// <summary>ARM64 NEON-accelerated RGB555→BGRA: 8 pixels (16B→32B) per iteration.</summary>
    /// <remarks>RGB555 bit layout: xRRRRRGGGGGBBBBB (bit 15 unused).
    /// Differences from RGB565 NEON:
    ///   - Red:  >> 10 (not >> 11) — skips unused bit 15
    ///   - Green: &amp; 0x001F (5 bits, not 6)
    ///   - Green MSB-replication: (val &lt;&lt; 3) | (val &gt;&gt; 2) — 5→8 bits (same as R/B)</remarks>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static void DecodeRgb555_Neon(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian, bool swapRgbChannels)
        => DecodeRgbX_Neon(src, dst, w, h, littleEndian, swapRgbChannels, 10, 5, 0, 0x001F, 3, &DecodeRgb555_Tail);

    /// <summary>Scalar fallback for RGB555 decode.</summary>
    private static void DecodeRgb555_Scalar(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian, bool swapRgbChannels)
    {
        int rowStride = (int)((long)src.Length / h);
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
                DecodeRgb555_Tail(pSrc + (nint)(y * rowStride),
                    dst + y * w * 4, 0, w, littleEndian, swapRgbChannels);
        }
    }

    /// <summary>Inner row decoder — used by both SIMD (tail) and scalar paths.</summary>
    private static void DecodeRgb555_Tail(byte* pSrc, byte* pDstRow, int xStart, int w, bool littleEndian, bool swapRgbChannels)
    {
        byte* pDst = pDstRow + xStart * 4;
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
            pDst[0] = (byte)((b5 << 3) | (b5 >> 2));
            pDst[1] = (byte)((g5 << 3) | (g5 >> 2));
            pDst[2] = (byte)((r5 << 3) | (r5 >> 2));
            pDst[3] = 255;
            pDst += 4;
        }
    }

    // ---------- Reordered RGB555 (quad-tree / REC_RGB555) ----------
    //
    // Apple's recursive-ordered dither format used by iPhone/Touch cover art
    // (profiles 3001/3002/3003). Pixels are stored in Morton Z-order (interleaved
    // bit pattern) rather than raster scanline order. De-derange reconstructs
    // raster-order pixels, then each is decoded as standard RGB555→BGRA8.
    // The quadtree level is determined by the image dimension (must be power-of-2, square).
    /// <summary>Returns false when the input buffer is too small or dimensions are invalid.</summary>
    internal static bool DecodeReorderedRgb555(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        if (w <= 0 || h <= 0) return false;
        if (w != h) return false; // REC_RGB555 only used for square images in practice
        if ((w & (w - 1)) != 0) return false; // must be power of 2
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return false;

        int bits = System.Numerics.BitOperations.Log2((uint)w);
        // Temp buffer for de-deranged RGB555 pixel data (raster order)
        byte[] temp = new byte[w * h * 2];
        fixed (byte* pSrc = src)
        fixed (byte* pTemp = temp)
        {
            for (int y = 0; y < h; y++)
            {
                for (int x = 0; x < w; x++)
                {
                    // Compute Morton Z-order index from (x, y)
                    uint z = MortonInterleave((uint)x, (uint)y, bits);
                    int srcIdx = (int)z * 2;
                    int dstIdx = (y * w + x) * 2;
                    pTemp[dstIdx] = pSrc[srcIdx];
                    pTemp[dstIdx + 1] = pSrc[srcIdx + 1];
                }
            }
        }
        // Decode reordered data as plain RGB555
        return DecodeRgb555(temp, dst, w, h, littleEndian);
    }

    /// <summary>Interleaves bits of x and y into Morton Z-order (y0 x0 y1 x1 ...).</summary>
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static uint MortonInterleave(uint x, uint y, int bits)
    {
        uint z = 0;
        for (int i = 0; i < bits; i++)
        {
            z |= ((x >> i) & 1) << (2 * i + 1);
            z |= ((y >> i) & 1) << (2 * i);
        }
        return z;
    }

}
