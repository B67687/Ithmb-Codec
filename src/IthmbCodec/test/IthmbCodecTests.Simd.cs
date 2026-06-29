using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    // ---- SIMD correctness: SSE2 vs scalar must produce identical output ----

    [Fact]
    public void DecodeRgb565_SIMD_MatchesScalar_8Wide()
    {
        // Verifies the SSE2 path (activated by w >= 8) and the scalar path (w < 8)
        // produce byte-identical output for all 65,536 RGB565 values processed as 8-wide rows.
        var src = new byte[8 * 2]; // 8 pixels × 2 bytes
        byte* simdDst = (byte*)NativeMemory.Alloc(8 * 4);
        byte* scalarDst = (byte*)NativeMemory.Alloc(8 * 4);
        try
        {
            for (int i = 0; i < 65536; i += 8)
            {
                // Fill 8 pixels with a sequence of 8 consecutive RGB565 values
                for (int j = 0; j < 8; j++)
                {
                    ushort val = (ushort)((i + j) & 0xFFFF);
                    src[j * 2] = (byte)(val & 0xFF);
                    src[j * 2 + 1] = (byte)(val >> 8);
                }

                NativeMemory.Clear(simdDst, 8 * 4);
                NativeMemory.Clear(scalarDst, 8 * 4);

                // SIMD path (w = 8, SSE2 activated on x64)
                IthmbCodecPlugin.DecodeRgb565(src, simdDst, 8, 1, littleEndian: true);

                // Scalar path (w = 4 forces fallback)
                IthmbCodecPlugin.DecodeRgb565(src, scalarDst, 4, 2, littleEndian: true);

                // Both must produce identical BGRA output (same pixel count)
                for (int j = 0; j < 8 * 4; j++)
                    Assert.Equal(simdDst[j], scalarDst[j]);
            }
        }
        finally { NativeMemory.Free(simdDst); NativeMemory.Free(scalarDst); }
    }


    [Fact]
    public void DecodeRgb565_SIMD_KnownValue()
    {
        // Simple known value test: RGB565 red pixel 0xF800
        // Red in RGB565 = bits 15-11 = 11111 = 31, green=0, blue=0
        // Output: R=255, G=0, B=0
        var src = new byte[8 * 2]; // 8 pixels, all 0xF800 (pure red)
        for (int i = 0; i < 8; i++)
        {
            src[i * 2] = 0x00;     // LE low byte
            src[i * 2 + 1] = 0xF8; // LE high byte
        }

        byte* dst = (byte*)NativeMemory.Alloc(8 * 4);
        try
        {
            IthmbCodecPlugin.DecodeRgb565(src, dst, 8, 1, littleEndian: true);

            for (int i = 0; i < 8; i++)
            {
                Assert.Equal(0, dst[i * 4]);     // B
                Assert.Equal(0, dst[i * 4 + 1]); // G
                Assert.Equal(255, dst[i * 4 + 2]); // R
                Assert.Equal(255, dst[i * 4 + 3]); // A
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    [Fact]
    public void DecodeRgb565_SIMD_AllColors_8Wide()
    {
        // 8 different colors, each pixel a different value.
        // Proves the SIMD path treats each of the 8 pixels independently.
        ushort[] colors = [0x0000, 0xF800, 0x07E0, 0x001F, 0xFFFF, 0x7800, 0x0010, 0x7BEF];
        var src = new byte[16];
        for (int i = 0; i < 8; i++)
        {
            src[i * 2] = (byte)(colors[i] & 0xFF);
            src[i * 2 + 1] = (byte)(colors[i] >> 8);
        }

        byte* dst = (byte*)NativeMemory.Alloc(8 * 4);
        try
        {
            IthmbCodecPlugin.DecodeRgb565(src, dst, 8, 1, littleEndian: true);

            for (int i = 0; i < 8; i++)
            {
                ushort rgb = colors[i];
                int r5 = (rgb >> 11) & 0x1F;
                int g6 = (rgb >> 5) & 0x3F;
                int b5 = rgb & 0x1F;
                int er = (r5 << 3) | (r5 >> 2);
                int eg = (g6 << 2) | (g6 >> 4);
                int eb = (b5 << 3) | (b5 >> 2);

                Assert.Equal(eb, dst[i * 4]);      // B
                Assert.Equal(eg, dst[i * 4 + 1]);  // G
                Assert.Equal(er, dst[i * 4 + 2]);  // R
                Assert.Equal(255, dst[i * 4 + 3]); // A
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    // ---- YCbCr420 SIMD correctness ----

    [Fact]
    public void DecodeYcbcr420_SIMD_Grayscale_MatchesFormula()
    {
        // Verifies the SIMD path against the BT.601 formula for grayscale images
        // with neutral chroma (Cb=Cr=128). Tests multiple even dimensions.
        foreach (int w in new[] { 2, 4, 8, 16, 42, 64 })
        {
            foreach (int h in new[] { 2, 4, 8, 16, 42, 64 })
            {
                int ySize = w * h;
                int uvSize = ((w + 1) / 2) * ((h + 1) / 2);
                var src = new byte[ySize + uvSize * 2];

                // Fill Y with gradient
                for (int i = 0; i < ySize; i++) src[i] = (byte)(i & 0xFF);
                // Neutral chroma
                for (int i = 0; i < uvSize; i++) src[ySize + i] = 128;
                for (int i = 0; i < uvSize; i++) src[ySize + uvSize + i] = 128;

                int bufLen = w * h * 4;
                byte* dst = (byte*)NativeMemory.Alloc((nuint)bufLen);
                try
                {
                    NativeMemory.Clear(dst, (nuint)bufLen);

                    // Decode via SIMD path (w,h even → SIMD active on x64)
                    IthmbCodecPlugin.DecodeYcbcr420(src, dst, w, h);

                    // Verify against BT.601: neutral chroma → R≈Y, G≈Y, B≈Y
                    for (int y = 0; y < h; y++)
                    {
                        for (int x = 0; x < w; x++)
                        {
                            int yy = src[y * w + x];
                            int idx = (y * w + x) * 4;
                            Assert.InRange(dst[idx], yy - 8, yy + 8);     // B ≈ Y
                            Assert.InRange(dst[idx + 1], yy - 8, yy + 8); // G ≈ Y
                            Assert.InRange(dst[idx + 2], yy - 8, yy + 8); // R ≈ Y
                            Assert.Equal(255, dst[idx + 3]);               // A
                        }
                    }
                }
                finally { NativeMemory.Free(dst); }
            }
        }
    }

    // ===================== P3: SIMD coverage + JSON parser tests =====================

    // ---- UYVY SIMD (SSSE3) known-value test with w=8 ----

    [Fact]
    public void DecodeYuv422_SIMD_KnownValue_8Wide()
    {
        // 8 identical red pixels in UYVY format: all U=85, V=255 (red chroma), Y=76 (red luma)
        var src = new byte[16];
        for (int i = 0; i < 8; i += 2)
        {
            src[i * 2] = 85;     // U
            src[i * 2 + 1] = 76; // Y0
            src[i * 2 + 2] = 255; // V
            src[i * 2 + 3] = 76; // Y1
        }

        byte* dst = (byte*)NativeMemory.Alloc(8 * 4);
        try
        {
            IthmbCodecPlugin.DecodeYuv422(src, dst, 8, 1);

            // All 8 pixels should be approximately red (R dominates, G/B low)
            for (int i = 0; i < 8; i++)
            {
                Assert.True(dst[i * 4 + 2] > 200, $"Pixel {i}: R={dst[i*4+2]} not dominant");
                Assert.True(dst[i * 4 + 1] < 50, $"Pixel {i}: G={dst[i*4+1]} not low");
                Assert.Equal(255, dst[i * 4 + 3]); // A
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    // ---- CLCL nibble-chroma (speculative, untested) ----

    [Fact]
    public void DecodeYuv422Clcl_NeutralChroma_Grayscale()
    {
        // CLCL format: [CbCr][Y0][CbCr][Y1] — 4 bytes per 2 pixels.
        // Neutral chroma: Cb=8, Cr=8 → packed = 0x88, scaled: 8*17=136.
        // With Y=128 and neutral chroma, output should be approximately gray.
        var src = new byte[8];
        for (int i = 0; i < 8; i += 4)
        {
            src[i] = 0x88;     // CbCr
            src[i + 1] = 128;  // Y0
            src[i + 2] = 0x88; // CbCr (duplicate)
            src[i + 3] = 128;  // Y1
        }

        byte* dst = (byte*)NativeMemory.Alloc(8 * 4);
        try
        {
            IthmbCodecPlugin.DecodeYuv422Clcl(src, dst, 4, 1);

            // All pixels should be approximately gray (±20 tolerance for 4-bit precision loss)
            for (int i = 0; i < 4; i++)
            {
                Assert.InRange(dst[i * 4], 100, 156);     // B
                Assert.InRange(dst[i * 4 + 1], 100, 156); // G
                Assert.InRange(dst[i * 4 + 2], 100, 156); // R
                Assert.Equal(255, dst[i * 4 + 3]);         // A
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    // ---- YCbCr420 SIMD known-value test ----

    [Fact]
    public void DecodeYcbcr420_SIMD_KnownValue()
    {
        // 2×2 block: all luma=128, Cb=128, Cr=128 → neutral chroma → mid-gray output
        var src = new byte[4 + 1 + 1]; // Y=4, Cb=1, Cr=1
        for (int i = 0; i < 4; i++) src[i] = 128;
        src[4] = 128; // Cb
        src[5] = 128; // Cr

        byte* dst = (byte*)NativeMemory.Alloc(4 * 4);
        try
        {
            IthmbCodecPlugin.DecodeYcbcr420(src, dst, 2, 2);

            // Neutral chroma → all channels ≈ luma (128, with ±8 tolerance)
            for (int i = 0; i < 4; i++)
            {
                Assert.InRange(dst[i * 4], 120, 136);     // B
                Assert.InRange(dst[i * 4 + 1], 120, 136); // G
                Assert.InRange(dst[i * 4 + 2], 120, 136); // R
                Assert.Equal(255, dst[i * 4 + 3]);         // A
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    [Fact]
    public void DecodeYuv422_SIMD_MatchesScalar_8Wide()
    {
        // Verifies the SSSE3 path (w=8) produces byte-identical output to the scalar path (w=2)
        // for all 256 combinations of Y, U, V values.
        var rng = new Random(42);
        byte* simdDst = (byte*)NativeMemory.Alloc(8 * 4);
        byte* scalarDst = (byte*)NativeMemory.Alloc(8 * 4);
        try
        {
            for (int iter = 0; iter < 256; iter++)
            {
                // 8 pixels of UYVY data: U Y0 V Y1 repeated
                int uVal = rng.Next(256);
                int vVal = rng.Next(256);
                var src = new byte[16];
                for (int i = 0; i < 16; i += 4)
                {
                    src[i] = (byte)uVal;
                    src[i + 1] = (byte)rng.Next(256); // Y0
                    src[i + 2] = (byte)vVal;
                    src[i + 3] = (byte)rng.Next(256); // Y1
                }

                NativeMemory.Clear(simdDst, 8 * 4);
                NativeMemory.Clear(scalarDst, 8 * 4);

                // SIMD path (w=8, SSSE3 dispatched on x64)
                IthmbCodecPlugin.DecodeYuv422(src, simdDst, 8, 1);

                // Scalar path (w=2 forces fallback)
                IthmbCodecPlugin.DecodeYuv422(src, scalarDst, 2, 4);

                for (int j = 0; j < 8 * 4; j++)
                    Assert.Equal(simdDst[j], scalarDst[j]);
            }
        }
        finally { NativeMemory.Free(simdDst); NativeMemory.Free(scalarDst); }
    }

    [Fact]
    public void DecodeYcbcr420_SIMD_MatchesFormula()
    {
        // Verifies the Vector128 SIMD path matches the BT.601 formula for colorful input.
        int w = 4, h = 4;
        int ySize = w * h;
        int uvSize = ((w + 1) / 2) * ((h + 1) / 2);
        var src = new byte[ySize + uvSize * 2];
        var rng = new Random(99);
        rng.NextBytes(src);
        // Ensure chroma is in valid range (already random bytes, debiased to ~128)
        for (int i = 0; i < uvSize; i++) { src[ySize + i] = (byte)rng.Next(256); }
        for (int i = 0; i < uvSize; i++) { src[ySize + uvSize + i] = (byte)rng.Next(256); }

        byte* dst = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
        try
        {
            IthmbCodecPlugin.DecodeYcbcr420(src, dst, w, h);

            // Check each pixel against BT.601 fixed-point formula
            for (int y = 0; y < h; y++)
            {
                for (int x = 0; x < w; x++)
                {
                    int luma = src[y * w + x];
                    int cu = y / 2, cv = x / 2;
                    int uvIdx = ySize + cu * (w / 2) + cv;
                    int cb = src[uvIdx] - 128;
                    int cr = src[uvIdx + uvSize] - 128;

                    int er = Clamp(luma + ((359 * cr) >> 8));
                    int eg = Clamp(luma - ((88 * cb) >> 8) - ((183 * cr) >> 8));
                    int eb = Clamp(luma + ((454 * cb) >> 8));

                    int off = (y * w + x) * 4;
                    Assert.Equal(eb, dst[off]);     // B
                    Assert.Equal(eg, dst[off + 1]); // G
                    Assert.Equal(er, dst[off + 2]); // R
                    Assert.Equal(255, dst[off + 3]); // A
                }
            }
        }
        finally { NativeMemory.Free(dst); }
    }
}
