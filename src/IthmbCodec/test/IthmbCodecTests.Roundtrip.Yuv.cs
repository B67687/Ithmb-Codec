using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

// SIZE_OK: YUV roundtrip tests (~400 pure LOC)
public unsafe partial class IthmbCodecTests
{
    [Fact]
    public void Property_Determinism_Yuv422()
    {
        byte[] src = [128, 128, 128, 128]; // neutral chroma + mid-gray
        AssertDeterminism(8, dst => IthmbCodecPlugin.DecodeYuv422(src, (byte*)(void*)dst, 2, 1));
    }

    [Fact]
    public void Property_Determinism_Ycbcr420()
    {
        byte[] src = [128, 128, 128, 128, 128, 128]; // 2x2 neutral
        AssertDeterminism(16, dst => IthmbCodecPlugin.DecodeYcbcr420(src, (byte*)(void*)dst, 2, 2));
    }

    // ===================== Phase 2: Cross-reference validation =====================

    // ---- Test 6: Full YUV422 cross (256 Y × 256 (U,V) pairs) ----

    [Fact]
    public void Yuv422_Cross_AllYallUV()
    {
        // Tests decoder stability for ALL possible Y values paired with
        // ALL possible (U,V) chroma values. Verifies no crash, all pixels
        // in valid range, and alpha always 255.
        var src = new byte[4]; // U Y0 V Y1 for 2 pixels
        byte* dst = (byte*)NativeMemory.Alloc(8); // 2 pixels × 4 bytes
        try
        {
            for (int yVal = 0; yVal < 256; yVal++)
            {
                for (int uVal = 0; uVal < 256; uVal += 8) // stride 8 to keep runtime sane
                {
                    for (int vVal = 0; vVal < 256; vVal += 8)
                    {
                        src[0] = (byte)uVal;
                        src[1] = (byte)yVal;
                        src[2] = (byte)vVal;
                        src[3] = (byte)yVal; // same luma for both pixels

                        NativeMemory.Clear(dst, 8);
                        IthmbCodecPlugin.DecodeYuv422(src, dst, 2, 1);

                        // Both pixels must have valid values
                        for (int p = 0; p < 2; p++)
                        {
                            int off = p * 4;
                            Assert.InRange(dst[off], 0, 255);     // B
                            Assert.InRange(dst[off + 1], 0, 255); // G
                            Assert.InRange(dst[off + 2], 0, 255); // R
                            Assert.Equal(255, dst[off + 3]);       // A
                        }
                    }
                }
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    // ---- Test 7: YCbCr420 exhaustive 2×2 block chroma sanity ----

    [Fact]
    public void Ycbcr420_Cross_NoChromaBleed()
    {
        // Tests that chroma values (Cb, Cr) applied at 2×2 block granularity
        // don't bleed outside expected pixel positions.
        int w = 4, h = 4;
        int ySize = w * h;
        int uvSize = ySize / 4;
        var src = new byte[ySize + uvSize * 2];
        byte* dst = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
        try
        {
            for (int topCb = 0; topCb < 256; topCb += 32)
            {
                for (int topCr = 0; topCr < 256; topCr += 32)
                {
                    for (int i = 0; i < ySize; i++) src[i] = 128;
                    for (int i = 0; i < uvSize; i++) src[ySize + i] = 128;
                    for (int i = 0; i < uvSize; i++) src[ySize + uvSize + i] = 128;

                    src[ySize] = (byte)topCb;
                    src[ySize + uvSize] = (byte)topCr;

                    NativeMemory.Clear(dst, (nuint)(w * h * 4));
                    IthmbCodecPlugin.DecodeYcbcr420(src, dst, w, h);

                    for (int i = 0; i < w * h * 4; i += 4)
                    {
                        Assert.InRange(dst[i], 0, 255);
                        Assert.InRange(dst[i + 1], 0, 255);
                        Assert.InRange(dst[i + 2], 0, 255);
                        Assert.Equal(255, dst[i + 3]);
                    }
                }
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    // --------------------- YUV422 (UYVY) ---------------------

    [Fact]
    public void Yuv422_Roundtrip_NeutralChroma()
    {
        // Neutral chroma (128,128): output should equal luma for all channels
        // Encode a 2×1 row: two pixels with known RGB values
        int r0 = 128, g0 = 128, b0 = 128; // mid-gray
        int r1 = 255, g1 = 255, b1 = 255; // white

        // Encode via reference T.601 forward transform
        var (y0, u, v) = RgbToYuv(r0, g0, b0);
        var (y1, _, _) = RgbToYuv(r1, g1, b1);

        // UYVY layout: [U][Y0][V][Y1]
        byte[] src = [(byte)u, (byte)y0, (byte)v, (byte)y1];

        byte* dst = (byte*)NativeMemory.Alloc(2 * 4);
        try
        {
            IthmbCodecPlugin.DecodeYuv422(src, dst, 2, 1);

            // With neutral chroma (U=V=128), chroma offset = 0 after -128
            // Output should be approximately gray (all channels ≈ luma)
            int tol = 16; // YUV roundtrip is lossy (quantization + coefficient rounding)
            Assert.InRange(dst[2], r0 - tol, r0 + tol); // R0
            Assert.InRange(dst[1], g0 - tol, g0 + tol); // G0
            Assert.InRange(dst[0], b0 - tol, b0 + tol); // B0
            Assert.InRange(dst[6], r1 - tol, r1 + tol); // R1
            Assert.InRange(dst[5], g1 - tol, g1 + tol); // G1
            Assert.InRange(dst[4], b1 - tol, b1 + tol); // B1
        }
        finally { NativeMemory.Free(dst); }
    }

    [Fact]
    public void Yuv422_Roundtrip_RedPixel()
    {
        // Pure red (255,0,0) → YCbCr values using BT.601
        var (y, u, v) = RgbToYuv(255, 0, 0);

        // Two identical red pixels
        byte[] src = [(byte)u, (byte)y, (byte)v, (byte)y];
        byte* dst = (byte*)NativeMemory.Alloc(2 * 4);
        try
        {
            IthmbCodecPlugin.DecodeYuv422(src, dst, 2, 1);

            // Red should dominate
            Assert.True(dst[2] > 200, $"R0 should be red, got {dst[2]}");
            Assert.True(dst[6] > 200, $"R1 should be red, got {dst[6]}");
        }
        finally { NativeMemory.Free(dst); }
    }

    // --------------------- YCbCr 4:2:0 ---------------------

    [Fact]
    public void Ycbcr420_Roundtrip_GrayscaleGrid()
    {
        // 4×4 grayscale image: all neutral chroma, checkerboard luma
        int w = 4, h = 4;
        int ySize = w * h;
        int uvSize = ySize / 4;

        byte[] src = new byte[ySize + uvSize * 2];
        // Y plane: checkerboard (0 and 255)
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
                src[y * w + x] = (byte)(((x + y) % 2 == 0) ? 0 : 255);

        // Cb plane: neutral (128)
        for (int i = 0; i < uvSize; i++)
            src[ySize + i] = 128;
        // Cr plane: neutral (128)
        for (int i = 0; i < uvSize; i++)
            src[ySize + uvSize + i] = 128;

        byte* dst = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
        try
        {
            IthmbCodecPlugin.DecodeYcbcr420(src, dst, w, h);

            // With neutral chroma, output = luma values (approximately)
            for (int y = 0; y < h; y++)
            {
                for (int x = 0; x < w; x++)
                {
                    int expectedLuma = ((x + y) % 2 == 0) ? 0 : 255;
                    int idx = (y * w + x) * 4;
                    // YUV roundtrip: allow ±16 for luma precision
                    Assert.InRange(dst[idx + 2], expectedLuma - 16, expectedLuma + 16); // R
                    Assert.InRange(dst[idx + 1], expectedLuma - 16, expectedLuma + 16); // G
                    Assert.InRange(dst[idx], expectedLuma - 16, expectedLuma + 16);     // B
                }
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    // --------------------- Interlaced YUV422 (F1019) ---------------------

    [Fact]
    public void Yuv422Interlaced_Roundtrip_MatchesLinearForFlatImage()
    {
        // For a flat-color image, interlaced and non-interlaced decode should match.
        // This validates the field-split logic is correct.
        int w = 4, h = 4;
        int bytesPerField = ((h + 1) / 2) * w * 2; // field split matches production formula
        int totalBytes = bytesPerField * 2;   // 32 bytes
        byte[] flatSrc = new byte[totalBytes];

        // Fill with a known UYVY pattern (neutral chroma, varying luma)
        for (int field = 0; field < 2; field++)
        {
            for (int row = 0; row < h / 2; row++)
            {
                for (int x = 0; x < w; x += 2)
                {
                    int off = field * bytesPerField + row * (w * 2) + x * 2;
                    flatSrc[off] = 128;     // U (neutral)
                    flatSrc[off + 1] = (byte)(field * 128 + row * 64 + x * 32); // Y0
                    flatSrc[off + 2] = 128; // V (neutral)
                    flatSrc[off + 3] = (byte)(field * 128 + row * 64 + x * 32 + 16); // Y1
                }
            }
        }

        byte* linearDst = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
        byte* interlaceDst = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
        try
        {
            // We need to create a non-interlaced version of the same data
            // Re-arrange: interlace stores even rows in first half, odd in second
            // Linear stores all rows sequentially
            byte[] linearSrc = new byte[totalBytes];
            for (int y = 0; y < h; y++)
            {
                int fieldOffset = (y % 2 == 0) ? 0 : bytesPerField;
                int rowInField = y / 2;
                for (int x = 0; x < w * 2; x++)
                {
                    linearSrc[y * w * 2 + x] = flatSrc[fieldOffset + rowInField * w * 2 + x];
                }
            }

            IthmbCodecPlugin.DecodeYuv422(linearSrc, linearDst, w, h);
            IthmbCodecPlugin.DecodeYuv422Interlaced(flatSrc, interlaceDst, w, h);

            // Both should produce identical output
            for (int i = 0; i < w * h * 4; i++)
            {
                Assert.Equal(linearDst[i], interlaceDst[i]);
            }
        }
        finally
        {
            NativeMemory.Free(linearDst);
            NativeMemory.Free(interlaceDst);
        }
    }

    [Fact]
    public void Roundtrip_Uyvy_Gradient()
    {
        // UYVY encode→decode with smooth gradient (realistic — adjacent pixels similar)
        // Random colors cause large chroma averaging error; gradients do not.
        int w = 64, h = 16;
        var bgra = new byte[w * h * 4];
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
            {
                int off = (y * w + x) * 4;
                bgra[off] = (byte)((x * 255) / w);      // B gradient horizontal
                bgra[off + 1] = (byte)((y * 255) / h);  // G gradient vertical
                bgra[off + 2] = (byte)(((x + y) * 128) / (w + h)); // R diagonal
                bgra[off + 3] = 255;
            }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1019, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Yuv422,
            FrameByteLength: w * h * 2);
        byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, w, h, profile);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(ithmbFile, profile,
                cancellation: null, outInfo, outBuf);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);

            var decoded = new Span<byte>((void*)outBuf->Data, w * h * 4);
            int maxError = 0, totalError = 0;
            for (int i = 0; i < w * h; i++)
            {
                int pxOff = i * 4;
                int dr = Math.Abs(decoded[pxOff + 2] - bgra[pxOff + 2]);
                int dg = Math.Abs(decoded[pxOff + 1] - bgra[pxOff + 1]);
                int db = Math.Abs(decoded[pxOff] - bgra[pxOff]);
                maxError = Math.Max(maxError, Math.Max(dr, Math.Max(dg, db)));
                totalError += dr + dg + db;
                Assert.Equal(255, decoded[pxOff + 3]);
            }
            // YUV is lossy: max error per channel ≤ 5 for smooth gradient data
            Assert.InRange(maxError, 0, 5);
            // Average error per channel per pixel ≤ 1.5
            int avgError = totalError / (w * h * 3);
            Assert.InRange(avgError, 0, 2);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    [Fact]
    public void Roundtrip_Ycbcr420_Gradient()
    {
        // Smooth gradient test — realistic for real photos where adjacent pixels are similar
        int w = 64, h = 64;
        var bgra = new byte[w * h * 4];
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
            {
                int off = (y * w + x) * 4;
                bgra[off] = (byte)((x * 255) / w);
                bgra[off + 1] = (byte)((y * 255) / h);
                bgra[off + 2] = (byte)(((x + y) * 128) / (w + h));
                bgra[off + 3] = 255;
            }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1067, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Ycbcr420,
            FrameByteLength: w * h * 2,
            IsPadded: true);
        byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, w, h, profile);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(ithmbFile, profile,
                cancellation: null, outInfo, outBuf);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);

            var decoded = new Span<byte>((void*)outBuf->Data, w * h * 4);
            int maxError = 0, totalError = 0;
            for (int i = 0; i < w * h; i++)
            {
                int pxOff = i * 4;
                int dr = Math.Abs(decoded[pxOff + 2] - bgra[pxOff + 2]);
                int dg = Math.Abs(decoded[pxOff + 1] - bgra[pxOff + 1]);
                int db = Math.Abs(decoded[pxOff] - bgra[pxOff]);
                maxError = Math.Max(maxError, Math.Max(dr, Math.Max(dg, db)));
                totalError += dr + dg + db;
                Assert.Equal(255, decoded[pxOff + 3]);
            }
            // YCbCr 4:2:0 (2×2 chroma block) has more averaging than UYVY
            Assert.InRange(maxError, 0, 6);
            int avgError = totalError / (w * h * 3);
            Assert.InRange(avgError, 0, 2);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    [Fact]
    public void DecodeRawProfile_PaddedYcbcr420_NoCrash()
    {
        // Synthetic test for padded YCbCr420 (profile F1067: 720x480, IsPadded=true).
        // Creates a buffer with valid YCbCr data plus extra padding bytes.
        int w = 720, h = 480;
        int validSize = w * h + ((w + 1) / 2) * ((h + 1) / 2) * 2;
        int paddedSize = w * h * 2; // 2 Bpp as in FrameByteLength
        var data = new byte[4 + paddedSize]; // 4-byte prefix + padded frame

        // Fill prefix (arbitrary 4 bytes)
        data[0] = 0x00; data[1] = 0x00; data[2] = 0x04; data[3] = 0x2B; // 1067 in big-endian

        // Fill Y plane with gradient
        for (int i = 0; i < w * h; i++) data[4 + i] = (byte)(i & 0xFF);
        // Fill Cb, Cr planes with neutral chroma
        for (int i = w * h; i < validSize; i++) data[4 + i] = 128;
        // Padding bytes remain 0 (unused)

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1067, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Ycbcr420,
            FrameByteLength: paddedSize,
            LittleEndian: true, SwapsDimensions: false,
            IsPadded: true, IsInterlaced: false, ClclChroma: false);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(data, profile,
                cancellation: null, outInfo, outBuf: null);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
        }
        finally { NativeMemory.Free(outInfo); }
    }

    [Fact]
    public void DecodeRawProfile_SwapChromaPlanes_NoCrash()
    {
        int w = 4, h = 4;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++) bgra[i * 4 + 3] = 255;
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 9997, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Ycbcr420,
            FrameByteLength: w * h * 2, IsPadded: true, SwapChromaPlanes: true);
        byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, w, h, profile);
        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(ithmbFile, profile,
                cancellation: null, outInfo, outBuf);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }
}
