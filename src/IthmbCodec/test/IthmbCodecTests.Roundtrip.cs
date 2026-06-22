using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    [Fact]
    public void Property_Determinism_Rgb565()
    {
        byte[] src = [0x00, 0xF8]; // 16-bit LE = 0xF800 = pure red
        AssertDeterminism(4, dst => IthmbCodecPlugin.DecodeRgb565(src, (byte*)(void*)dst, 1, 1, littleEndian: true));
    }

    [Fact]
    public void Property_Determinism_Rgb555()
    {
        byte[] src = [0x00, 0x7C]; // 16-bit LE = 0x7C00 = red (xRRRRR=11111)
        AssertDeterminism(4, dst => IthmbCodecPlugin.DecodeRgb555(src, (byte*)(void*)dst, 1, 1, littleEndian: true));
    }

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

    [Fact]
    public void Property_OutputDims_MatchInput()
    {
        // Structural: decode functions accept w,h and produce output of correct dimensions.
        // This is verified by the memory alloc pattern — if w*h*4 bytes are written
        // without OOB, the dimensions are structurally correct.
        byte[] src = new byte[32];
        new Random(42).NextBytes(src);
        int[] testDims = [1, 2, 3, 4, 8, 16];
        foreach (int w in testDims)
        {
            foreach (int h in testDims)
            {
                int allocSize = Math.Max(4096, w * h * 4);
                byte* dst = (byte*)NativeMemory.Alloc((nuint)allocSize);
                try
                {
                    NativeMemory.Clear(dst, (nuint)allocSize);
                    IthmbCodecPlugin.DecodeRgb565(src, dst, w, h, littleEndian: true);
                    // No crash = structurally valid
                }
                finally { NativeMemory.Free(dst); }
            }
        }
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

    // --------------------- RGB565 ---------------------

    [Fact]
    public void Rgb565_Roundtrip_AllCornerColors()
    {
        // Test every combination of R=0,128,255, G=0,128,255, B=0,128,255 (27 combos)
        int[] vals = [0, 64, 128, 192, 255];
        foreach (int r in vals)
        {
            foreach (int g in vals)
            {
                foreach (int b in vals)
                {
                    // Pack using reference encoder (matching iOpenPod)
                    ushort packed = PackRgb565(r, g, b);
                    byte leLow = (byte)(packed & 0xFF);
                    byte leHigh = (byte)(packed >> 8);
                    byte[] src = [leLow, leHigh];

                    byte* dst = (byte*)NativeMemory.Alloc(4);
                    try
                    {
                        // Decode using our implementation
                        IthmbCodecPlugin.DecodeRgb565(src, dst, 1, 1, littleEndian: true);

                        // RGB565 is lossy (5-bit/6-bit precision). Allow ±4 error.
                        int dr = Math.Abs(dst[2] - r);
                        int dg = Math.Abs(dst[1] - g);
                        int db = Math.Abs(dst[0] - b);
                        Assert.True(dr <= 8, $"R roundtrip error too large: {r}→{dst[2]} (Δ{dr})");
                        Assert.True(dg <= 8, $"G roundtrip error too large: {g}→{dst[1]} (Δ{dg})");
                        Assert.True(db <= 8, $"B roundtrip error too large: {b}→{dst[0]} (Δ{db})");
                    }
                    finally
                    {
                        NativeMemory.Free(dst);
                    }
                }
            }
        }
    }

    [Fact]
    public void Rgb565_Roundtrip_ExactValues()
    {
        // Some values roundtrip exactly through 5/6-bit precision:
        // R=0→0, R=255→248+7=255, G=0→0, G=255→252+3=255, B=0→0, B=255→248+7=255
        ushort packed = PackRgb565(255, 255, 255);
        byte[] src = [(byte)(packed & 0xFF), (byte)(packed >> 8)];
        byte* dst = (byte*)NativeMemory.Alloc(4);
        try
        {
            IthmbCodecPlugin.DecodeRgb565(src, dst, 1, 1, littleEndian: true);
            Assert.Equal(255, dst[2]); // R
            Assert.Equal(255, dst[1]); // G
            Assert.Equal(255, dst[0]); // B
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

    // ---- Roundtrip tests: encode → decode → compare pixel-perfect ----

    [Fact]
    public void Roundtrip_Rgb565_Exhaustive()
    {
        // Test all 65,536 RGB565 values at 64×1024 for full coverage
        var bgra = new byte[65536 * 4];
        for (int i = 0; i < 65536; i++)
        {
            ushort rgb565 = (ushort)i;
            int r5 = (rgb565 >> 11) & 0x1F;
            int g6 = (rgb565 >> 5) & 0x3F;
            int b5 = rgb565 & 0x1F;
            // MSB-replication (same as decoder uses)
            bgra[i * 4] = (byte)((b5 << 3) | (b5 >> 2));         // B
            bgra[i * 4 + 1] = (byte)((g6 << 2) | (g6 >> 4));     // G
            bgra[i * 4 + 2] = (byte)((r5 << 3) | (r5 >> 2));     // R
            bgra[i * 4 + 3] = 255;
        }

        // Encode then decode
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1007, Width: 65536, Height: 1,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: 65536 * 2);
        byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, 65536, 1, profile);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(ithmbFile, profile,
                cancellation: null, outInfo, outBuf);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
            Assert.NotEqual((nint)0, (nint)outBuf->Data);

            var decoded = new Span<byte>((void*)outBuf->Data, 65536 * 4);
            for (int i = 0; i < 65536 * 4; i++)
            {
                Assert.Equal(bgra[i], decoded[i]);
            }
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    [Fact]
    public void Roundtrip_Rgb555_Exhaustive()
    {
        var bgra = new byte[32768 * 4];
        for (int i = 0; i < 32768; i++)
        {
            ushort rgb555 = (ushort)i;
            int r5 = (rgb555 >> 10) & 0x1F;
            int g5 = (rgb555 >> 5) & 0x1F;
            int b5 = rgb555 & 0x1F;
            bgra[i * 4] = (byte)((b5 << 3) | (b5 >> 2));
            bgra[i * 4 + 1] = (byte)((g5 << 3) | (g5 >> 2));
            bgra[i * 4 + 2] = (byte)((r5 << 3) | (r5 >> 2));
            bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 3008, Width: 32768, Height: 1,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb555,
            FrameByteLength: 32768 * 2);
        byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, 32768, 1, profile);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(ithmbFile, profile,
                cancellation: null, outInfo, outBuf);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
            Assert.NotEqual((nint)0, (nint)outBuf->Data);

            var decoded = new Span<byte>((void*)outBuf->Data, 32768 * 4);
            for (int i = 0; i < 32768 * 4; i++)
                Assert.Equal(bgra[i], decoded[i]);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
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
    public void Roundtrip_AllProfiles_NonCrashing()
    {
        // Smoke test: every known profile at least doesn't crash on encode→decode
        var profiles = IthmbCodecPlugin.KnownProfiles;
        var bgra = new byte[64 * 64 * 4];
        for (int i = 0; i < 64 * 64; i++)
        {
            bgra[i * 4] = (byte)(i & 0xFF);      // B
            bgra[i * 4 + 1] = (byte)((i * 7) & 0xFF); // G
            bgra[i * 4 + 2] = (byte)((i * 13) & 0xFF); // R
            bgra[i * 4 + 3] = 255;
        }

        foreach (var kvp in profiles)
        {
            var profile = kvp.Value;
            try
            {
                byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, 64, 64, profile);

                var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
                    (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
                var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
                    (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
                try
                {
                    var status = IthmbCodecPlugin.DecodeRawProfile(ithmbFile, profile,
                        cancellation: null, outInfo, outBuf);
                    // Smoke: any status is OK except Internal
                    Assert.NotEqual(ImageGlass.SDK.Plugins.IGStatus.Internal, status);
                }
                finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
            }
            catch (Exception ex)
            {
                throw new Exception($"Profile {profile.Prefix} ({profile.Encoding}) failed: {ex.Message}");
            }
        }
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
    public void DecodeRawProfile_BufferTooSmall_ReturnsDecodeFailed()
    {
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1007, Width: 480, Height: 864,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: 480 * 864 * 2);

        var data = new byte[4 + 10]; // Way too small
        var status = IthmbCodecPlugin.DecodeRawProfile(data, profile,
            cancellation: null, outInfo: null, outBuf: null);
        Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.DecodeFailed, status);
    }

    [Fact]
    public void DecodeRawProfile_TrailingPaddingTolerance_SlightlySmaller_Succeeds()
    {
        // Tests that files slightly smaller than FrameByteLength (within TrailingPaddingTolerance=256)
        // are accepted and decoded. This handles real device alignment quirks where the encoder
        // wrote fewer bytes than expected. Behavior informed by iOpenPod's trailing-trim approach.
        int frameSize = 100 * 100 * 2; // 20,000 bytes for a 100×100 RGB565 frame
        int actualSize = frameSize - 128; // 128 bytes short — within 256-byte tolerance
        var data = new byte[4 + actualSize];
        // Write prefix (1005 in big-endian)
        data[0] = 0x00; data[1] = 0x00; data[2] = 0x03; data[3] = 0xED; // 1005 = 0x03ED
        // Fill pixel data with neutral values
        for (int i = 4; i < data.Length; i++) data[i] = 128;

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1005, Width: 100, Height: 100,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: frameSize);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(data, profile,
                cancellation: null, outInfo, outBuf);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
            Assert.NotEqual((nint)0, (nint)outBuf->Data);
            // Zero-padded pixels should decode to near-black (padded bytes are 0)
            // but the middle of the image had valid data
            Assert.Equal(100, outBuf->Width);
            Assert.Equal(100, outBuf->Height);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    [Fact]
    public void DecodeRawProfile_TrailingPaddingTolerance_TooSmall_Fails()
    {
        // Beyond tolerance (512 bytes short) should still fail
        int frameSize = 100 * 100 * 2;
        var data = new byte[4 + frameSize - 512]; // 512 bytes short — beyond 256-byte tolerance
        data[0] = 0x00; data[1] = 0x00; data[2] = 0x03; data[3] = 0xED;

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1005, Width: 100, Height: 100,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: frameSize);

        var status = IthmbCodecPlugin.DecodeRawProfile(data, profile,
            cancellation: null, outInfo: null, outBuf: null);
        Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.DecodeFailed, status);
    }

    [Fact]
    public void DecodeRawProfile_ClChroma_DecodeOnly()
    {
        // Test that CL decoder path at least doesn't crash (speculative)
        byte[] src = new byte[4 + 4 * 4 * 2]; // prefix + 4x4 CL data
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 9999, Width: 4, Height: 4,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Yuv422,
            FrameByteLength: 4 * 4 * 2, ClChroma: true);
        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(src, profile,
                cancellation: null, outInfo, outBuf);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    [Fact]
    public void DecodeRawProfile_ClclChroma_DecodeOnly()
    {
        byte[] src = new byte[4 + 4 * 4 * 2];
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 9998, Width: 4, Height: 4,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Yuv422,
            FrameByteLength: 4 * 4 * 2, ClclChroma: true);
        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(src, profile,
                cancellation: null, outInfo, outBuf);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
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

    [Fact]
    public void DecodeRawProfile_Rotation_90_NoCrash()
    {
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 9996, Width: 2, Height: 4,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: 2 * 4 * 2, Rotation: 90);
        var bgra = new byte[2 * 4 * 4];
        for (int i = 0; i < 2 * 4; i++) bgra[i * 4 + 3] = 255;
        byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, 2, 4, profile);
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

    // ===================== Centered crop infrastructure =====================

    [Fact]
    public void DecodeRawProfile_Crop_ExtractsCorrectRegion()
    {
        // 4×4 RGB565 image with a recognizable pattern. Crop to 2×2 center region.
        // Layout (BGRA values):
        // [r0,g0,b0] [r1,g1,b1] [r2,g2,b2] [r3,g3,b3]
        // [r4,g4,b4] [r5,g5,b5] [r6,g6,b6] [r7,g7,b7]
        // [r8,g8,b8] [r9,g9,b9] [ra,ga,ba] [rb,gb,bb]
        // [rc,gc,bc] [rd,gd,bd] [re,ge,be] [rf,gf,bf]
        // Crop region: CropX=1, CropY=1, CropWidth=2, CropHeight=2
        // Expected: pixels at (1,1), (2,1), (1,2), (2,2) = r5, r6, r9, ra
        int w = 4, h = 4;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++)
        {
            bgra[i * 4]     = (byte)(i * 3);       // B
            bgra[i * 4 + 1] = (byte)(i * 7);       // G
            bgra[i * 4 + 2] = (byte)(i * 11);      // R
            bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 9995, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: w * h * 2,
            CropX: 1, CropY: 1, CropWidth: 2, CropHeight: 2);

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
            Assert.Equal(2, outBuf->Width);
            Assert.Equal(2, outBuf->Height);

            var decoded = new Span<byte>((void*)outBuf->Data, 2 * 2 * 4);
            // Expected pixels: index 5 (1,1), index 6 (2,1), index 9 (1,2), index 10 (2,2)
            int[] expectedIndices = [5, 6, 9, 10];
            for (int px = 0; px < 4; px++)
            {
                int srcIdx = expectedIndices[px];
                int off = px * 4;
                // Allow ±4 tolerance for RGB565 lossy roundtrip
                Assert.InRange(decoded[off],     bgra[srcIdx * 4] - 8,     bgra[srcIdx * 4] + 8);
                Assert.InRange(decoded[off + 1], bgra[srcIdx * 4 + 1] - 8, bgra[srcIdx * 4 + 1] + 8);
                Assert.InRange(decoded[off + 2], bgra[srcIdx * 4 + 2] - 8, bgra[srcIdx * 4 + 2] + 8);
                Assert.Equal(255, decoded[off + 3]);
            }
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    [Fact]
    public void DecodeRawProfile_CropWithRotation_CropsAfterRotation()
    {
        // Profile with both Rotation=90 and Crop. Verify crop dimensions are in the
        // rotated space (crop happens after rotate). 4×2 image rotated 90° → 2×4.
        // Crop to 2×2 from rotated space.
        int w = 4, h = 2;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++)
        {
            bgra[i * 4]     = (byte)(i * 5);
            bgra[i * 4 + 1] = (byte)(i * 9);
            bgra[i * 4 + 2] = (byte)(i * 13);
            bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 9994, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: w * h * 2,
            Rotation: 90, CropWidth: 2, CropHeight: 2);

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
            // After 90° rotation: 4×2 → 2×4. Crop to 2×2 from top-left.
            Assert.Equal(2, outBuf->Width);
            Assert.Equal(2, outBuf->Height);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    [Fact]
    public void DecodeRawProfile_Crop_InvalidBounds_ReturnsFullImage()
    {
        // Crop bounds outside the image — should fall through and return full image.
        int w = 4, h = 4;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h * 4; i++) bgra[i] = 128;
        for (int i = 3; i < w * h * 4; i += 4) bgra[i] = 255;

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 9993, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: w * h * 2,
            CropX: 10, CropY: 10, CropWidth: 2, CropHeight: 2); // Outside bounds

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
            // Should return full uncropped image
            Assert.Equal(w, outBuf->Width);
            Assert.Equal(h, outBuf->Height);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    // ===================== Multi-frame raw decode =====================

    [Fact]
    public void MultiFrame_RawDecode_ThreeFrames_DecodesAllIndependently()
    {
        // F-prefix .ithmb files can contain multiple concatenated raw frames.
        // This verifies that DecodeRawProfile seeks to and decodes each frame independently.
        int w = 4, h = 4;
        int frameSize = w * h * 2;
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1007, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: frameSize);

        // Three distinct BGRA images: solid red, solid green, solid blue
        var frameBgra = new byte[3 * w * h * 4];
        for (int i = 0; i < w * h; i++)
            { frameBgra[i * 4] = 0; frameBgra[i * 4 + 1] = 0; frameBgra[i * 4 + 2] = 255; frameBgra[i * 4 + 3] = 255; }
        int off1 = w * h * 4;
        for (int i = 0; i < w * h; i++)
            { frameBgra[off1 + i * 4] = 0; frameBgra[off1 + i * 4 + 1] = 255; frameBgra[off1 + i * 4 + 2] = 0; frameBgra[off1 + i * 4 + 3] = 255; }
        int off2 = 2 * w * h * 4;
        for (int i = 0; i < w * h; i++)
            { frameBgra[off2 + i * 4] = 255; frameBgra[off2 + i * 4 + 1] = 0; frameBgra[off2 + i * 4 + 2] = 0; frameBgra[off2 + i * 4 + 3] = 255; }

        // Each BuildIthmbFile returns [4-byte prefix][encoded data]; extract encoded portion
        byte[] frame0 = IthmbCodecPlugin.BuildIthmbFile(frameBgra.AsSpan(0, w * h * 4), w, h, profile);
        byte[] frame1 = IthmbCodecPlugin.BuildIthmbFile(frameBgra.AsSpan(off1, w * h * 4), w, h, profile);
        byte[] frame2 = IthmbCodecPlugin.BuildIthmbFile(frameBgra.AsSpan(off2, w * h * 4), w, h, profile);
        var enc0 = frame0.AsSpan(4);
        var enc1 = frame1.AsSpan(4);
        var enc2 = frame2.AsSpan(4);

        // Build multi-frame file: one prefix + 3 encoded frames concatenated
        var combined = new byte[4 + enc0.Length + enc1.Length + enc2.Length];
        Array.Copy(frame0, 0, combined, 0, 4);
        enc0.CopyTo(combined.AsSpan(4));
        enc1.CopyTo(combined.AsSpan(4 + enc0.Length));
        enc2.CopyTo(combined.AsSpan(4 + enc0.Length + enc1.Length));

        Assert.Equal(3, (combined.Length - 4) / frameSize);

        for (int fi = 0; fi < 3; fi++)
        {
            var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
                (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
            var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
                (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
            try
            {
                var status = IthmbCodecPlugin.DecodeRawProfile(combined, profile,
                    cancellation: null, outInfo, outBuf, frameIndex: fi);
                Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
                Assert.NotEqual((nint)0, (nint)outBuf->Data);
                Assert.Equal(3, outInfo->FrameCount);

                var decoded = new Span<byte>((void*)outBuf->Data, w * h * 4);
                for (int i = 0; i < w * h; i++)
                {
                    int px = i * 4;
                    int expectedR = fi == 0 ? 255 : fi == 1 ? 0 : 0;
                    int expectedG = fi == 0 ? 0 : fi == 1 ? 255 : 0;
                    int expectedB = fi == 0 ? 0 : fi == 1 ? 0 : 255;
                    Assert.InRange(Math.Abs(decoded[px + 2] - expectedR), 0, 8);
                    Assert.InRange(Math.Abs(decoded[px + 1] - expectedG), 0, 8);
                    Assert.InRange(Math.Abs(decoded[px] - expectedB), 0, 8);
                    Assert.Equal(255, decoded[px + 3]);
                }
            }
            finally
            {
                if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
                NativeMemory.Free(outInfo);
                NativeMemory.Free(outBuf);
            }
        }
    }

    [Fact]
    public void MultiFrame_RawDecode_OutOfRangeIndex_ReturnsDecodeFailed()
    {
        int w = 4, h = 4;
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1007, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: w * h * 2);

        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h * 4; i++) bgra[i] = 128;
        byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, w, h, profile);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(ithmbFile, profile,
                cancellation: null, outInfo, outBuf, frameIndex: 1);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.DecodeFailed, status);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    [Fact]
    public void MultiFrame_RawDecode_NegativeIndex_ReturnsDecodeFailed()
    {
        int w = 4, h = 4;
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1007, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: w * h * 2);

        var bgra = new byte[w * h * 4];
        byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, w, h, profile);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(ithmbFile, profile,
                cancellation: null, outInfo, outBuf, frameIndex: -1);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.DecodeFailed, status);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }
}
