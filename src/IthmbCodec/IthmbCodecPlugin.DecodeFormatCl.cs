// CL per-pixel nibble-chroma YCbCr 4:2:2 decoder for .ithmb raw profiles.
using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Intrinsics;
using System.Runtime.Intrinsics.Arm;
using System.Runtime.Intrinsics.X86;

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

        if (Ssse3.IsSupported && (w & 7) == 0)
            DecodeYuv422Cl_SIMD(src, dst, w, h);
        else if (AdvSimd.IsSupported && (w & 7) == 0)
            DecodeYuv422Cl_Neon(src, dst, w, h);
        else
            DecodeYuv422Cl_Scalar(src, dst, w, h);
        return true;
    }

    private static void DecodeYuv422Cl_Scalar(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
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
    }

    // ---- SIMD constants ----

    /// <summary>pshufb mask: extract Y from CL bytes (odd positions 1,3,5,7,9,11,13,15).</summary>
    private static readonly Vector128<byte> ClShufY = Vector128.Create(
        (byte)1, 0x80, 3, 0x80, 5, 0x80, 7, 0x80, 9, 0x80, 11, 0x80, 13, 0x80, 15, 0x80);

    /// <summary>pshufb mask: extract CbCr bytes from CL (even positions 0,2,4,6,8,10,12,14).</summary>
    private static readonly Vector128<byte> ClShufC = Vector128.Create(
        (byte)0, 0x80, 2, 0x80, 4, 0x80, 6, 0x80, 8, 0x80, 10, 0x80, 12, 0x80, 14, 0x80);

    /// <summary>SSSE3-accelerated CL→BGRA (8 pixels per iteration).</summary>
    private static void DecodeYuv422Cl_SIMD(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        var shufY = ClShufY;
        var shufC = ClShufC;
        var zeroI = Vector128<int>.Zero;
        var max255 = Vector128.Create(255);
        var alpha = Vector128.Create(255 << 24);
        var rCoef = Vector128.Create(YuvRCoef);
        var gCoefCb = Vector128.Create(YuvGCoefCb);
        var gCoefCr = Vector128.Create(YuvGCoefCr);
        var bCoef = Vector128.Create(YuvBCoef);

        int rowStride = (int)((long)src.Length / h);
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pRow = pSrc + y * rowStride;
                byte* pDstRow = dst + y * w * 4;
                for (int x = 0; x < w; x += 8)
                {
                    // Load 16 bytes = 8 pixels
                    var raw = Vector128.LoadUnsafe(ref *pRow, (nuint)(x * 2));

                    // Y values at odd byte positions
                    var y16 = Ssse3.Shuffle(raw, shufY).AsInt16();

                    // CbCr bytes at even byte positions (each pixel has its own)
                    var cbcr16 = Ssse3.Shuffle(raw, shufC).AsInt16();

                    // Cb = high nibble * 16
                    var cb16 = Sse2.And(cbcr16, Vector128.Create(unchecked((short)0xFFF0)));
                    // Cr = low nibble * 16
                    var cr16 = Sse2.ShiftLeftLogical(
                        Sse2.And(cbcr16, Vector128.Create((short)0x000F)), 4);

                    var zero16 = Vector128<short>.Zero;
                    for (int hIdx = 0; hIdx < 2; hIdx++)
                    {
                        var yI = (hIdx == 0
                            ? Sse2.UnpackLow(y16, zero16)
                            : Sse2.UnpackHigh(y16, zero16)).AsInt32();
                        var cbI = (hIdx == 0
                            ? Sse2.UnpackLow(cb16, zero16)
                            : Sse2.UnpackHigh(cb16, zero16)).AsInt32();
                        var crI = (hIdx == 0
                            ? Sse2.UnpackLow(cr16, zero16)
                            : Sse2.UnpackHigh(cr16, zero16)).AsInt32();

                        cbI = Sse2.Subtract(cbI, Vector128.Create(128));
                        crI = Sse2.Subtract(crI, Vector128.Create(128));

                        var rI = yI + Vector128.ShiftRightArithmetic(crI * rCoef, 8);
                        var gI = yI - Vector128.ShiftRightArithmetic(cbI * gCoefCb, 8)
                                   - Vector128.ShiftRightArithmetic(crI * gCoefCr, 8);
                        var bI = yI + Vector128.ShiftRightArithmetic(cbI * bCoef, 8);

                        rI = Vector128.Max(zeroI, Vector128.Min(rI, max255));
                        gI = Vector128.Max(zeroI, Vector128.Min(gI, max255));
                        bI = Vector128.Max(zeroI, Vector128.Min(bI, max255));

                        var px = bI | Vector128.ShiftLeft(gI, 8)
                                    | Vector128.ShiftLeft(rI, 16) | alpha;

                        int* pRowOut = (int*)(pDstRow + x * 4 + hIdx * 16);
                        pRowOut[0] = px.GetElement(0);
                        pRowOut[1] = px.GetElement(1);
                        pRowOut[2] = px.GetElement(2);
                        pRowOut[3] = px.GetElement(3);
                    }
                }
            }
        }
    }

    /// <summary>ARM64 NEON-accelerated CL→BGRA.</summary>
    private static void DecodeYuv422Cl_Neon(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        var shufY = ClShufY;
        var shufC = ClShufC;
        var zeroI = Vector128<int>.Zero;
        var max255 = Vector128.Create(255);
        var alpha = Vector128.Create(255 << 24);
        var rCoef = Vector128.Create(YuvRCoef);
        var gCoefCb = Vector128.Create(YuvGCoefCb);
        var gCoefCr = Vector128.Create(YuvGCoefCr);
        var bCoef = Vector128.Create(YuvBCoef);

        int rowStride = (int)((long)src.Length / h);
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pRow = pSrc + y * rowStride;
                byte* pDstRow = dst + y * w * 4;
                for (int x = 0; x < w; x += 8)
                {
                    var raw = Vector128.LoadUnsafe(ref *pRow, (nuint)(x * 2));
                    var y16 = AdvSimd.Arm64.VectorTableLookup(raw, shufY).AsInt16();
                    var cbcr16 = AdvSimd.Arm64.VectorTableLookup(raw, shufC).AsInt16();

                    var cb16 = (cbcr16 & Vector128.Create(unchecked((short)0xFFF0))).AsInt16();
                    var cr16 = ((cbcr16 & Vector128.Create((short)0x000F)) << 4).AsInt16();

                    var zero16 = Vector128<short>.Zero;
                    for (int hIdx = 0; hIdx < 2; hIdx++)
                    {
                        var yI = (hIdx == 0
                            ? AdvSimd.Arm64.ZipLow(y16, zero16)
                            : AdvSimd.Arm64.ZipHigh(y16, zero16)).AsInt32();
                        var cbI = (hIdx == 0
                            ? AdvSimd.Arm64.ZipLow(cb16, zero16)
                            : AdvSimd.Arm64.ZipHigh(cb16, zero16)).AsInt32();
                        var crI = (hIdx == 0
                            ? AdvSimd.Arm64.ZipLow(cr16, zero16)
                            : AdvSimd.Arm64.ZipHigh(cr16, zero16)).AsInt32();

                        cbI = AdvSimd.Subtract(cbI, Vector128.Create(128));
                        crI = AdvSimd.Subtract(crI, Vector128.Create(128));

                        var rI = yI + Vector128.ShiftRightArithmetic(crI * rCoef, 8);
                        var gI = yI - Vector128.ShiftRightArithmetic(cbI * gCoefCb, 8)
                                   - Vector128.ShiftRightArithmetic(crI * gCoefCr, 8);
                        var bI = yI + Vector128.ShiftRightArithmetic(cbI * bCoef, 8);

                        rI = Vector128.Max(zeroI, Vector128.Min(rI, max255));
                        gI = Vector128.Max(zeroI, Vector128.Min(gI, max255));
                        bI = Vector128.Max(zeroI, Vector128.Min(bI, max255));

                        var px = bI | Vector128.ShiftLeft(gI, 8)
                                    | Vector128.ShiftLeft(rI, 16) | alpha;

                        int* pRowOut = (int*)(pDstRow + x * 4 + hIdx * 16);
                        pRowOut[0] = px.GetElement(0);
                        pRowOut[1] = px.GetElement(1);
                        pRowOut[2] = px.GetElement(2);
                        pRowOut[3] = px.GetElement(3);
                    }
                }
            }
        }
    }
}
