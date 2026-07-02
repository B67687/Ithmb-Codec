// Decode algorithms for .ithmb raw profiles — UYVY / YUV 4:2:2.
// SIZE_OK: UYVY 4:2:2 + Interlaced UYVY — SSSE3, NEON, and scalar paths; ISA duplication inflates LOC.
using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Intrinsics;
using System.Runtime.Intrinsics.Arm;
using System.Runtime.Intrinsics.X86;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    /// <summary>SIMD constants shared across UYVY row-processing methods.</summary>
    private readonly record struct UyvySimdConstants(
        Vector128<byte> ShufY, Vector128<byte> ShufU, Vector128<byte> ShufV,
        Vector128<int> ZeroI, Vector128<int> Max255, Vector128<int> Alpha,
        Vector128<int> RCoef, Vector128<int> GCoefCb, Vector128<int> GCoefCr, Vector128<int> BCoef);

    // ---------- YUV 4:2:2 (UYVY) ----------

    /// <summary>
    /// Decode a standard UYVY YUV 4:2:2 frame (2 Bpp).
    /// Common across iPod Classic 5G/6G, Nano 1G–4G (format IDs 1019, 1024, etc.).
    /// </summary>
    /// <remarks>
    /// Byte layout per 2-pixel macropixel (UYVY order):
    /// <code>
    ///   [Cb] [Y0] [Cr] [Y1]   — 4 bytes, 2 pixels
    /// </code>
    /// Cb and Cr are shared between the two pixels (4:2:2 subsampling).
    /// Conversion uses BT.601 coefficients.
    /// SIMD accelerated: SSSE3 (x86) and NEON (ARM64) via pshufb/VTBL.
    /// </remarks>
    internal static bool DecodeYuv422(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        if (w <= 0 || h <= 0) return false;
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
        int rowStride = (int)((long)src.Length / h);
        for (int y = 0; y < h; y++)
        {
            int rowStart = y * rowStride;
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

        int rowStride = (int)((long)src.Length / h);
        var c = new UyvySimdConstants(shufY, shufU, shufV, zeroI, max255, alpha,
            rCoef, gCoefCb, gCoefCr, bCoef);
        fixed (byte* pSrc = src)
            for (int y = 0; y < h; y++)
                ProcessUyvyRow(pSrc + (nint)(y * rowStride), dst + (nint)(y * w * 4), w, c);
    }

    /// <summary>Processes one row of UYVY data using SSSE3/SSE2 with 32-bit arithmetic (8 pixels per iteration).</summary>
    /// <remarks>32-bit Vector128&lt;int&gt;.Multiply, Max, Min map to pmulld/pmaxsd/pminsd (SSE4.1+).</remarks>
    private static void ProcessUyvyRow(byte* pSrcRow, byte* pDstRow, int w, UyvySimdConstants c)
    {
        for (int x = 0; x < w; x += 8)
        {
            var raw = Vector128.LoadUnsafe(ref *pSrcRow, (nuint)(x * 2));
            var y16 = Ssse3.Shuffle(raw, c.ShufY).AsInt16();
            var u16 = Ssse3.Shuffle(raw, c.ShufU).AsInt16();
            var v16 = Ssse3.Shuffle(raw, c.ShufV).AsInt16();

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

                uI = Sse2.Subtract(uI, Vector128.Create(128));
                vI = Sse2.Subtract(vI, Vector128.Create(128));

                var rI = yI + Vector128.ShiftRightArithmetic(vI * c.RCoef, 8);
                var gI = yI - Vector128.ShiftRightArithmetic(uI * c.GCoefCb, 8)
                           - Vector128.ShiftRightArithmetic(vI * c.GCoefCr, 8);
                var bI = yI + Vector128.ShiftRightArithmetic(uI * c.BCoef, 8);

                rI = Vector128.Max(c.ZeroI, Vector128.Min(rI, c.Max255));
                gI = Vector128.Max(c.ZeroI, Vector128.Min(gI, c.Max255));
                bI = Vector128.Max(c.ZeroI, Vector128.Min(bI, c.Max255));

                var px = bI | Vector128.ShiftLeft(gI, 8)
                    | Vector128.ShiftLeft(rI, 16) | c.Alpha;

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

        int rowStride = (int)((long)src.Length / h);
        var c = new UyvySimdConstants(shufY, shufU, shufV, zeroI, max255, alpha,
            rCoef, gCoefCb, gCoefCr, bCoef);
        fixed (byte* pSrc = src)
            for (int y = 0; y < h; y++)
                ProcessUyvyRow_Neon(pSrc + (nint)(y * rowStride), dst + (nint)(y * w * 4), w, c);
    }

    /// <summary>Processes one row of UYVY data using NEON AdvSimd with VectorTableLookup (TBL / pshufb equivalent).</summary>
    /// <remarks>Mirrors ProcessUyvyRow but uses AdvSimd intrinsics for the architecture-specific parts.</remarks>
    private static void ProcessUyvyRow_Neon(byte* pSrcRow, byte* pDstRow, int w, UyvySimdConstants c)
    {
        for (int x = 0; x < w; x += 8)
        {
            var raw = Vector128.LoadUnsafe(ref *pSrcRow, (nuint)(x * 2));

            // VectorTableLookup (Arm64 overload) = ARM TBL = SSSE3 pshufb — same shuffle masks.
            // The Vector128,Vector128 overload is AdvSimd.Arm64-only (full 128-bit TBL).
            var y16 = AdvSimd.Arm64.VectorTableLookup(raw, c.ShufY).AsInt16();
            var u16 = AdvSimd.Arm64.VectorTableLookup(raw, c.ShufU).AsInt16();
            var v16 = AdvSimd.Arm64.VectorTableLookup(raw, c.ShufV).AsInt16();

            var zero16 = Vector128<short>.Zero;
            for (int hIdx = 0; hIdx < 2; hIdx++)
            {
                var yI = (hIdx == 0
                    ? AdvSimd.Arm64.ZipLow(y16, zero16)
                    : AdvSimd.Arm64.ZipHigh(y16, zero16)).AsInt32();
                var uI = (hIdx == 0
                    ? AdvSimd.Arm64.ZipLow(u16, zero16)
                    : AdvSimd.Arm64.ZipHigh(u16, zero16)).AsInt32();
                var vI = (hIdx == 0
                    ? AdvSimd.Arm64.ZipLow(v16, zero16)
                    : AdvSimd.Arm64.ZipHigh(v16, zero16)).AsInt32();

                uI = AdvSimd.Subtract(uI, Vector128.Create(128));
                vI = AdvSimd.Subtract(vI, Vector128.Create(128));

                var rI = yI + Vector128.ShiftRightArithmetic(vI * c.RCoef, 8);
                var gI = yI - Vector128.ShiftRightArithmetic(uI * c.GCoefCb, 8)
                           - Vector128.ShiftRightArithmetic(vI * c.GCoefCr, 8);
                var bI = yI + Vector128.ShiftRightArithmetic(uI * c.BCoef, 8);

                rI = Vector128.Max(c.ZeroI, Vector128.Min(rI, c.Max255));
                gI = Vector128.Max(c.ZeroI, Vector128.Min(gI, c.Max255));
                bI = Vector128.Max(c.ZeroI, Vector128.Min(bI, c.Max255));

                var px = bI | Vector128.ShiftLeft(gI, 8)
                    | Vector128.ShiftLeft(rI, 16) | c.Alpha;

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
        int rowStride = (int)((long)src.Length / h);
        int half = ((h + 1) / 2) * rowStride;

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

        var c = new UyvySimdConstants(shufY, shufU, shufV, zeroI, max255, alpha,
            rCoef, gCoefCb, gCoefCr, bCoef);
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                int fieldOffset = (y % 2 == 0) ? 0 : half;
                int rowInField = y / 2;
                int rowStart = fieldOffset + rowInField * rowStride;
                ProcessUyvyRow_Neon(pSrc + rowStart, dst + (nint)(y * w * 4), w, c);
            }
        }
    }

    /// <summary>Returns false when the input buffer is too small (defensive guard).</summary>
    internal static bool DecodeYuv422Interlaced(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        if (w <= 0 || h <= 0) return false;
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
        int rowStride = (int)((long)src.Length / h);
        int half = ((h + 1) / 2) * rowStride;
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
        int rowStride = (int)((long)src.Length / h);
        int half = ((h + 1) / 2) * rowStride;

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

        var c = new UyvySimdConstants(shufY, shufU, shufV, zeroI, max255, alpha,
            rCoef, gCoefCb, gCoefCr, bCoef);
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                int fieldOffset = (y % 2 == 0) ? 0 : half;
                int rowInField = y / 2;
                int rowStart = fieldOffset + rowInField * rowStride;
                ProcessUyvyRow(pSrc + rowStart, dst + (nint)(y * w * 4), w, c);
            }
        }
    }
}
