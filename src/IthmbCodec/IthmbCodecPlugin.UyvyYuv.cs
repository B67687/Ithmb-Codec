// Decode algorithms for .ithmb raw profiles — UYVY / YUV 4:2:2.
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
}
