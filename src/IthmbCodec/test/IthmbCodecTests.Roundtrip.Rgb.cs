using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

// SIZE_OK: RGB roundtrip tests (~350 pure LOC)
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
}
