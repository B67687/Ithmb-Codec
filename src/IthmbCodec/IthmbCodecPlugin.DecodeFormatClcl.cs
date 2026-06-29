// CLCL nibble-chroma YCbCr 4:2:2 decoder for .ithmb raw profiles.
using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Intrinsics;
using System.Runtime.Intrinsics.Arm;
using System.Runtime.Intrinsics.X86;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
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

        if (Ssse3.IsSupported && (w & 7) == 0)
            DecodeYuv422Clcl_SIMD(src, dst, w, h);
        else if (AdvSimd.IsSupported && (w & 7) == 0)
            DecodeYuv422Clcl_Neon(src, dst, w, h);
        else
            DecodeYuv422Clcl_Scalar(src, dst, w, h);
        return true;
    }

    private static void DecodeYuv422Clcl_Scalar(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
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
    }

    // ---- SIMD constants ----

    /// <summary>pshufb mask: extract Y from CLCL bytes (positions 1,3,5,7,9,11,13,15).</summary>
    private static readonly Vector128<byte> ClclShufY = Vector128.Create(
        (byte)1, 0x80, 3, 0x80, 5, 0x80, 7, 0x80, 9, 0x80, 11, 0x80, 13, 0x80, 15, 0x80);

    /// <summary>pshufb mask: extract CbCr bytes from CLCL (positions 0,2,4,6,8,10,12,14)
    /// and replicate each to two adjacent 16-bit lanes (shared chroma for pixel pairs).</summary>
    private static readonly Vector128<byte> ClclShufC = Vector128.Create(
        (byte)0, 0x80, 0, 0x80, 4, 0x80, 4, 0x80, 8, 0x80, 8, 0x80, 12, 0x80, 12, 0x80);


    /// <summary>SSSE3-accelerated CLCL→BGRA (8 pixels per iteration).</summary>
    private static void DecodeYuv422Clcl_SIMD(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        var shufY = ClclShufY;
        var shufC = ClclShufC;
        var zeroI = Vector128<int>.Zero;
        var max255 = Vector128.Create(255);
        var alpha = Vector128.Create(255 << 24);
        var rCoef = Vector128.Create(YuvRCoef);
        var gCoefCb = Vector128.Create(YuvGCoefCb);
        var gCoefCr = Vector128.Create(YuvGCoefCr);
        var bCoef = Vector128.Create(YuvBCoef);

        // Nibble masks
        var hiNibble = Vector128.Create((byte)0xF0);
        var loNibble = Vector128.Create((byte)0x0F);

        int rowStride = (int)((long)src.Length / h);
        fixed (byte* pSrc = src)
        {
            for (int y = 0; y < h; y++)
            {
                byte* pRow = pSrc + y * rowStride;
                byte* pDstRow = dst + y * w * 4;
                for (int x = 0; x < w; x += 8)
                {
                    // Load 16 bytes = 4 macropixels = 8 pixels
                    var raw = Vector128.LoadUnsafe(ref *pRow, (nuint)(x * 2));

                    // Y values (positions 1,3,5,7,9,11,13,15)
                    var y16 = Ssse3.Shuffle(raw, shufY).AsInt16();

                    // CbCr bytes (positions 0,2,4,6,8,10,12,14) — each replicated to 2 lanes
                    var cbcr16 = Ssse3.Shuffle(raw, shufC).AsInt16();

                    // Cb = high nibble * 16 = value & 0xF0
                    var cb16 = Sse2.And(cbcr16, Vector128.Create(unchecked((short)0xFFF0)));
                    // Cr = low nibble * 16 = (value & 0x0F) << 4
                    var cr16 = Sse2.ShiftLeftLogical(
                        Sse2.And(cbcr16, Vector128.Create((short)0x000F)), 4);

                    // BT.601 conversion: process 4 pixels at a time (low/high halves)
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

                        // Chroma center: subtracted 128 already handled via the
                        // nibble extraction math (0-240 range starts at Cb*16)
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

    /// <summary>ARM64 NEON-accelerated CLCL→BGRA.</summary>
    private static void DecodeYuv422Clcl_Neon(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        var shufY = ClclShufY;
        var shufC = ClclShufC;
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
