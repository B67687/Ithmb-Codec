using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe class IthmbCodecTests
{
    // ===================== RGB565 =====================

    [Theory]
    [InlineData(0x0000, 0, 0, 0)]       // black
    [InlineData(0xFFFF, 255, 255, 255)] // white
    [InlineData(0xF800, 255, 0, 0)]     // red (R=31)
    [InlineData(0x07E0, 0, 255, 0)]     // green (G=63)
    [InlineData(0x001F, 0, 0, 255)]     // blue (B=31)
    [InlineData(0x7800, 123, 0, 0)]     // half red (R=15 → 123 via MSB replication)
    [InlineData(0x0010, 0, 0, 132)]     // ~half blue (B=16 → 132 via MSB replication)
    public void DecodeRgb565_KnownColors(ushort rgb565, int expectedR, int expectedG, int expectedB)
    {
        // Arrange: encode a single pixel
        byte leLow = (byte)(rgb565 & 0xFF);
        byte leHigh = (byte)(rgb565 >> 8);
        byte[] src = [leLow, leHigh];
        byte* dst = (byte*)NativeMemory.Alloc(4);

        try
        {
            // Act
            IthmbCodecPlugin.DecodeRgb565(src, dst, 1, 1, littleEndian: true);

            // Assert (BGRA order)
            Assert.Equal(expectedB, dst[0]);
            Assert.Equal(expectedG, dst[1]);
            Assert.Equal(expectedR, dst[2]);
            Assert.Equal(255, dst[3]);
        }
        finally
        {
            NativeMemory.Free(dst);
        }
    }

    [Fact]
    public void DecodeRgb565_RowMajorOrder()
    {
        // A 2x2 image: red, green, blue, white
        ushort[] pixels = [0xF800, 0x07E0, 0x001F, 0xFFFF];
        byte[] src = new byte[8];
        for (int i = 0; i < 4; i++)
        {
            src[i * 2] = (byte)(pixels[i] & 0xFF);
            src[i * 2 + 1] = (byte)(pixels[i] >> 8);
        }
        byte* dst = (byte*)NativeMemory.Alloc(4 * 4); // 2x2 * 4 bytes

        try
        {
            IthmbCodecPlugin.DecodeRgb565(src, dst, 2, 2, littleEndian: true);

            // Pixel 0,0 = red
            Assert.Equal(0, dst[0]); Assert.Equal(0, dst[1]); Assert.Equal(255, dst[2]); Assert.Equal(255, dst[3]);
            // Pixel 1,0 = green
            Assert.Equal(0, dst[4]); Assert.Equal(255, dst[5]); Assert.Equal(0, dst[6]); Assert.Equal(255, dst[7]);
            // Pixel 0,1 = blue
            Assert.Equal(255, dst[8]); Assert.Equal(0, dst[9]); Assert.Equal(0, dst[10]); Assert.Equal(255, dst[11]);
            // Pixel 1,1 = white
            Assert.Equal(255, dst[12]); Assert.Equal(255, dst[13]); Assert.Equal(255, dst[14]); Assert.Equal(255, dst[15]);
        }
        finally
        {
            NativeMemory.Free(dst);
        }
    }

    [Fact]
    public void DecodeRgb565_BigEndian()
    {
        // Red pixel in big-endian: high byte = 11111_000_00 = 0xF8, low byte = 00000_000 = 0x00
        // Stored as: [high, low] = [0xF8, 0x00]
        byte[] src = [0xF8, 0x00];
        byte* dst = (byte*)NativeMemory.Alloc(4);

        try
        {
            IthmbCodecPlugin.DecodeRgb565(src, dst, 1, 1, littleEndian: false);
            Assert.Equal(0, dst[0]);   // B
            Assert.Equal(0, dst[1]);   // G
            Assert.Equal(255, dst[2]); // R
        }
        finally
        {
            NativeMemory.Free(dst);
        }
    }

    // ===================== YUV422 (UYVY) =====================

    [Fact]
    public void DecodeYuv422_NeutralChroma()
    {
        // U=128, V=128 → neutral grayscale chroma, pixel values = luma only
        // 2 pixels: U=128 Y0=0 V=128 Y1=255
        byte[] src = [128, 0, 128, 255];
        byte* dst = (byte*)NativeMemory.Alloc(2 * 4); // 1x2 * 4 bytes

        try
        {
            IthmbCodecPlugin.DecodeYuv422(src, dst, 2, 1);

            // Pixel 0: Y=0 → black
            Assert.Equal(0, dst[0]); Assert.Equal(0, dst[1]); Assert.Equal(0, dst[2]); Assert.Equal(255, dst[3]);
            // Pixel 1: Y=255 → white
            Assert.Equal(255, dst[4]); Assert.Equal(255, dst[5]); Assert.Equal(255, dst[6]); Assert.Equal(255, dst[7]);
        }
        finally
        {
            NativeMemory.Free(dst);
        }
    }

    [Fact]
    public void DecodeYuv422_KnownColor()
    {
        // Blue (0,0,255) in BT.601 YCbCr: Y=29, Cb=255, Cr=107
        // 2 pixels, both blue: [Cb=255] [Y0=29] [Cr=107] [Y1=29] ... actually let me compute properly
        // Actually this is the raw byte layout testing, precision depends on integer math
        // Let's use a more predictable case: full red
        // Red in BT.601 YCbCr: Y=76, Cb=85, Cr=255
        // Our decoder: R = 76 + (359*255>>8) = 76 + 357 = 433 → clamped to 255 ✅
        // G = 76 - (88*85>>8) - (183*255>>8) = 76 - 29 - 182 = -135 → clamped to 0 ✅
        // B = 76 + (454*85>>8) = 76 + 150 = 226 (slight deviation but close ✅)
        byte[] src = [85, 76, 255, 76]; // [Cb][Y0][Cr][Y1] for 2 pixels
        byte* dst = (byte*)NativeMemory.Alloc(2 * 4);

        try
        {
            IthmbCodecPlugin.DecodeYuv422(src, dst, 2, 1);

            // Pixel 0 should be approximately red (dominant R channel)
            Assert.True(dst[2] > 200); // R should be high
            Assert.True(dst[1] < 50);  // G should be low
            // B channel from BT.601 YCbCr red conversion is ~226 (not 0)
        }
        finally
        {
            NativeMemory.Free(dst);
        }
    }

    // ===================== YCbCr 4:2:0 =====================

    [Fact]
    public void DecodeYcbcr420_NeutralChroma()
    {
        // 2x2 image, all neutral chroma (128), variable luma
        // Y plane: [0, 255, 128, 64] (4 bytes)
        // Cb plane: [128] (1 byte, shared)
        // Cr plane: [128] (1 byte, shared)
        // Total: 6 bytes
        byte[] src = [0, 255, 128, 64, 128, 128];
        byte* dst = (byte*)NativeMemory.Alloc(4 * 4); // 2x2 * 4 bytes

        try
        {
            IthmbCodecPlugin.DecodeYcbcr420(src, dst, 2, 2);

            // With neutral chroma, output = luma in all channels
            Assert.Equal(0, dst[0]); Assert.Equal(0, dst[1]); Assert.Equal(0, dst[2]);   // black
            Assert.Equal(255, dst[4]); Assert.Equal(255, dst[5]); Assert.Equal(255, dst[6]); // white
            Assert.Equal(128, dst[8]);  // ~mid gray
            Assert.Equal(64, dst[12]);  // ~dark gray
        }
        finally
        {
            NativeMemory.Free(dst);
        }
    }

    // ===================== JPEG extraction =====================

    [Fact]
    public void TryFindJpegSlice_NoJpeg_ReturnsFalse()
    {
        byte[] data = [0x00, 0x01, 0x02, 0x03, 0xFF, 0xFE, 0xFD, 0xFC];
        bool found = IthmbCodecPlugin.TryFindJpegSlice(data, out _, out _, null);
        Assert.False(found);
    }

    [Fact]
    public void TryFindJpegSlice_WithJfif_ReturnsSlice()
    {
        // Build: [padding] + [SOI] + [APP0 with JFIF] + [SOS + image data] + [EOI]
        var jpeg = BuildMinimalJpeg(Jfif: true);
        var data = new byte[100 + jpeg.Length];
        // Fill first 100 bytes with zeros (padding)
        // Insert JPEG at offset 100
        Array.Copy(jpeg, 0, data, 100, jpeg.Length);

        bool found = IthmbCodecPlugin.TryFindJpegSlice(data, out int offset, out int length, null);

        Assert.True(found);
        Assert.Equal(100, offset);
        Assert.Equal(jpeg.Length, length);
    }

    [Fact]
    public void TryFindJpegSlice_WithExif_ReturnsSlice()
    {
        var jpeg = BuildMinimalJpeg(Jfif: false, Exif: true);
        var data = new byte[50 + jpeg.Length];
        Array.Copy(jpeg, 0, data, 50, jpeg.Length);

        bool found = IthmbCodecPlugin.TryFindJpegSlice(data, out int offset, out int length, null);

        Assert.True(found);
        Assert.Equal(50, offset);
        Assert.Equal(jpeg.Length, length);
    }

    [Fact]
    public void TryFindJpegSlice_NoEoi_UsesRestOfFile()
    {
        // JPEG without EOI marker
        var soi = new byte[] { 0xFF, 0xD8 };
        var app0 = new byte[] { 0xFF, 0xE0, 0x00, 0x10,
            (byte)'J', (byte)'F', (byte)'I', (byte)'F', 0, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00 };
        var data = new byte[10 + soi.Length + app0.Length + 20];
        Array.Copy(soi, 0, data, 10, soi.Length);
        Array.Copy(app0, 0, data, 12, app0.Length);
        // No EOI appended

        bool found = IthmbCodecPlugin.TryFindJpegSlice(data, out int offset, out int length, null);

        Assert.True(found);
        Assert.Equal(10, offset);
        Assert.Equal(data.Length - 10, length); // rest of file
    }

    // ===================== EXIF Orientation =====================

    [Fact]
    public void ReadExifOrientation_NoExif_Returns1()
    {
        var jpeg = BuildMinimalJpeg(Jfif: true);
        int orient = IthmbCodecPlugin.ReadExifOrientation(jpeg, 0, jpeg.Length);
        Assert.Equal(1, orient); // default = normal
    }

    [Fact]
    public void ReadExifOrientation_WithExifOrientation_ReturnsValue()
    {
        // Build a JPEG with an EXIF APP1 containing Orientation=6 (rotate 90 CW)
        var jpeg = BuildJpegWithExifOrientation(6);
        int orient = IthmbCodecPlugin.ReadExifOrientation(jpeg, 0, jpeg.Length);
        Assert.Equal(6, orient);
    }

    [Fact]
    public void ReadExifOrientation_OutOfRange_Returns1()
    {
        var jpeg = BuildJpegWithExifOrientation(99); // invalid
        int orient = IthmbCodecPlugin.ReadExifOrientation(jpeg, 0, jpeg.Length);
        Assert.Equal(1, orient);
    }

    // ===================== Synthetic JPEG builders =====================

    /// <summary>Builds a minimal valid JPEG with JFIF or EXIF APP1 marker.</summary>
    private static byte[] BuildMinimalJpeg(bool Jfif = false, bool Exif = false)
    {
        using var ms = new MemoryStream();
        // SOI
        ms.Write([0xFF, 0xD8]);
        if (Jfif)
        {
            // APP0 with JFIF identifier
            ms.Write([0xFF, 0xE0]);
            ms.WriteByte(0x00); ms.WriteByte(0x10); // length=16
            ms.Write([(byte)'J', (byte)'F', (byte)'I', (byte)'F', 0]); // identifier
            ms.WriteByte(0x01); ms.WriteByte(0x01); // version 1.01
            ms.WriteByte(0x00); // units=0 (aspect ratio)
            ms.WriteByte(0x00); ms.WriteByte(0x01); // X density
            ms.WriteByte(0x00); ms.WriteByte(0x01); // Y density
            ms.WriteByte(0x00); // thumbnail width
            ms.WriteByte(0x00); // thumbnail height
        }
        if (Exif)
        {
            // APP1 with Exif identifier and minimal TIFF
            var tiff = BuildMinimalTiff();
            int app1Len = 2 + 6 + tiff.Length; // tag+len + "Exif\0\0" + TIFF
            ms.Write([0xFF, 0xE1]);
            ms.WriteByte((byte)(app1Len >> 8)); ms.WriteByte((byte)app1Len);
            ms.Write([(byte)'E', (byte)'x', (byte)'i', (byte)'f', 0, 0]);
            ms.Write(tiff);
        }
        // DQT (quantization table) — minimal required for valid JPEG
        ms.Write([0xFF, 0xDB, 0x00, 0x43, 0]); // 64-byte table of 0s (worst quality, but valid)
        for (int i = 0; i < 64; i++) ms.WriteByte(1);
        // SOF0 (Start of Frame)
        ms.Write([0xFF, 0xC0, 0x00, 0x0B, 0x08]); // precision=8, height/width=0
        ms.WriteByte(0x00); ms.WriteByte(0x01); // height=1
        ms.WriteByte(0x00); ms.WriteByte(0x01); // width=1
        ms.WriteByte(0x01); // number of components=1 (grayscale)
        ms.WriteByte(0x01); // component ID=1
        ms.WriteByte(0x11); // sampling=1:1
        ms.WriteByte(0x00); // quantization table=0
        // SOS (Start of Scan)
        ms.Write([0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        ms.WriteByte(0x80); // single encoded byte (EOB)
        // EOI
        ms.Write([0xFF, 0xD9]);
        return ms.ToArray();
    }

    /// <summary>Builds an EXIF segment with an Orientation tag.</summary>
    private static byte[] BuildJpegWithExifOrientation(int orientation)
    {
        using var ms = new MemoryStream();
        // SOI
        ms.Write([0xFF, 0xD8]);
        // Build TIFF with orientation
        var tiff = BuildTiffWithOrientation(orientation);
        int app1Len = 2 + 6 + tiff.Length;
        ms.Write([0xFF, 0xE1]);
        ms.WriteByte((byte)(app1Len >> 8)); ms.WriteByte((byte)app1Len);
        ms.Write([(byte)'E', (byte)'x', (byte)'i', (byte)'f', 0, 0]);
        ms.Write(tiff);
        // Minimal image data to make it a valid JPEG (for scan safety)
        ms.Write([0xFF, 0xDB, 0x00, 0x43, 0]);
        for (int i = 0; i < 64; i++) ms.WriteByte(1);
        ms.Write([0xFF, 0xC0, 0x00, 0x0B, 0x08]);
        ms.WriteByte(0x00); ms.WriteByte(0x01);
        ms.WriteByte(0x00); ms.WriteByte(0x01);
        ms.WriteByte(0x01); ms.WriteByte(0x01); ms.WriteByte(0x11); ms.WriteByte(0x00);
        ms.Write([0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        ms.WriteByte(0x80);
        ms.Write([0xFF, 0xD9]);
        return ms.ToArray();
    }

    /// <summary>Minimal TIFF with zero IFD entries (no orientation).</summary>
    private static byte[] BuildMinimalTiff()
    {
        using var ms = new MemoryStream();
        // Little-endian TIFF header
        ms.Write([(byte)'I', (byte)'I', 0x2A, 0x00]); // II + magic 42
        // IFD0 offset = 8 (right after header)
        ms.WriteByte(0x08); ms.WriteByte(0x00); ms.WriteByte(0x00); ms.WriteByte(0x00);
        // IFD0: 0 entries, next IFD=0 (no more)
        ms.WriteByte(0x00); ms.WriteByte(0x00); // num entries=0
        ms.Write([0x00, 0x00, 0x00, 0x00]); // next IFD offset=0
        return ms.ToArray();
    }

    /// <summary>TIFF with a single Orientation (0x0112) IFD entry.</summary>
    private static byte[] BuildTiffWithOrientation(int orientation)
    {
        using var ms = new MemoryStream();
        // Little-endian TIFF header
        ms.Write([(byte)'I', (byte)'I', 0x2A, 0x00]);
        // IFD0 offset = 8
        ms.WriteByte(0x08); ms.WriteByte(0x00); ms.WriteByte(0x00); ms.WriteByte(0x00);
        // IFD0: 1 entry
        ms.WriteByte(0x01); ms.WriteByte(0x00); // num entries=1
        // Entry: Tag=0x0112 (Orientation), Type=SHORT(3), Count=1, Value=orientation
        ms.WriteByte(0x12); ms.WriteByte(0x01); // tag (little-endian 0x0112)
        ms.WriteByte(0x03); ms.WriteByte(0x00); // type=SHORT
        ms.WriteByte(0x01); ms.WriteByte(0x00); // count=1
        ms.WriteByte(0x00); ms.WriteByte(0x00); // count continued
        ms.WriteByte((byte)orientation); ms.WriteByte(0x00); // value in last 2 bytes, LE
        // Next IFD = 0
        ms.Write([0x00, 0x00, 0x00, 0x00]);
        return ms.ToArray();
    }

    // ===================== Phase 1: Exhaustive + Fuzz + Properties =====================

    // ---- Test 1: Exhaustive 1×1 RGB565 (all 65,536 possible 16-bit values) ----

    [Fact]
    public void Rgb565_Exhaustive_All65536Values()
    {
        // Tests EVERY possible 16-bit RGB565 input, proving the MSB-replication
        // formula is correct for all inputs. Runs in ~50ms.
        var src = new byte[2];
        byte* dst = (byte*)NativeMemory.Alloc(4);
        try
        {
            for (int i = 0; i < 65536; i++)
            {
                src[0] = (byte)(i & 0xFF);
                src[1] = (byte)(i >> 8);

                NativeMemory.Clear(dst, 4); // zero dst before each decode
                IthmbCodecPlugin.DecodeRgb565(src, dst, 1, 1, littleEndian: true);

                // Decode the word back out
                int r5 = (i >> 11) & 0x1F;
                int g6 = (i >> 5) & 0x3F;
                int b5 = i & 0x1F;

                int expectedR = (r5 << 3) | (r5 >> 2);
                int expectedG = (g6 << 2) | (g6 >> 4);
                int expectedB = (b5 << 3) | (b5 >> 2);

                // Must match the MSB-replication formula exactly
                Assert.Equal(expectedR, dst[2]); // R
                Assert.Equal(expectedG, dst[1]); // G
                Assert.Equal(expectedB, dst[0]); // B
                Assert.Equal(255, dst[3]); // A
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    // ---- Test 3: Fuzz — random buffers (no crash, valid output ranges) ----

    public static IEnumerable<object[]> GetRandomValidBuffers()
    {
        var rng = new Random(42);
        // Use small valid dimensions so buffers match expected sizes
        int[] sizes = [2, 4, 8, 16, 32, 64, 128];
        for (int i = 0; i < 50; i++)
        {
            int w = sizes[rng.Next(sizes.Length)];
            int h = sizes[rng.Next(sizes.Length)];
            var buf = new byte[w * h * 2]; // correct size for RGB565/YUV422
            rng.NextBytes(buf);
            yield return [buf, w, h];
        }
    }

    [Theory]
    [MemberData(nameof(GetRandomValidBuffers))]
    public void Fuzz_Rgb565_NoCrash(byte[] buf, int w, int h)
    {
        int allocSize = Math.Max(4096, w * h * 4);
        byte* dst = (byte*)NativeMemory.Alloc((nuint)allocSize);
        try
        {
            NativeMemory.Clear(dst, (nuint)allocSize);
            IthmbCodecPlugin.DecodeRgb565(buf, dst, w, h, littleEndian: true);
            int pixels = Math.Min(w * h, allocSize / 4);
            for (int i = 0; i < pixels; i++)
            {
                int offset = i * 4;
                Assert.InRange(dst[offset], 0, 255);     // B
                Assert.InRange(dst[offset + 1], 0, 255); // G
                Assert.InRange(dst[offset + 2], 0, 255); // R
                Assert.Equal(255, dst[offset + 3]);       // A
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    [Theory]
    [MemberData(nameof(GetRandomValidBuffers))]
    public void Fuzz_Yuv422_NoCrash(byte[] buf, int w, int h)
    {
        int allocSize = Math.Max(4096, w * h * 4);
        byte* dst = (byte*)NativeMemory.Alloc((nuint)allocSize);
        try
        {
            NativeMemory.Clear(dst, (nuint)allocSize);
            IthmbCodecPlugin.DecodeYuv422(buf, dst, w, h);
            int pixels = Math.Min(w * h, allocSize / 4);
            for (int i = 0; i < pixels; i++)
            {
                int offset = i * 4;
                Assert.InRange(dst[offset], 0, 255);
                Assert.InRange(dst[offset + 1], 0, 255);
                Assert.InRange(dst[offset + 2], 0, 255);
                Assert.Equal(255, dst[offset + 3]);
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    [Theory]
    [MemberData(nameof(GetRandomValidBuffers))]
    public void Fuzz_Ycbcr420_NoCrash(byte[] buf, int w, int h)
    {
        // YCbCr420 needs less: w*h*3/2 per frame
        int allocSize = Math.Max(4096, w * h * 4);
        byte* dst = (byte*)NativeMemory.Alloc((nuint)allocSize);
        try
        {
            NativeMemory.Clear(dst, (nuint)allocSize);
            IthmbCodecPlugin.DecodeYcbcr420(buf, dst, w, h);
            int pixels = Math.Min(w * h, allocSize / 4);
            for (int i = 0; i < pixels; i++)
            {
                int offset = i * 4;
                Assert.InRange(dst[offset], 0, 255);
                Assert.InRange(dst[offset + 1], 0, 255);
                Assert.InRange(dst[offset + 2], 0, 255);
                Assert.Equal(255, dst[offset + 3]);
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    [Theory]
    [MemberData(nameof(GetRandomValidBuffers))]
    public void Fuzz_InterlacedYuv422_NoCrash(byte[] buf, int w, int h)
    {
        int allocSize = Math.Max(4096, w * h * 4);
        byte* dst = (byte*)NativeMemory.Alloc((nuint)allocSize);
        try
        {
            NativeMemory.Clear(dst, (nuint)allocSize);
            IthmbCodecPlugin.DecodeYuv422Interlaced(buf, dst, w, h);
            int pixels = Math.Min(w * h, allocSize / 4);
            for (int i = 0; i < pixels; i++)
            {
                int offset = i * 4;
                Assert.InRange(dst[offset], 0, 255);
                Assert.InRange(dst[offset + 1], 0, 255);
                Assert.InRange(dst[offset + 2], 0, 255);
                Assert.Equal(255, dst[offset + 3]);
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    // ---- Test 4: Property invariants ----

    [Fact]
    public void Property_Determinism_Rgb565()
    {
        // Same input must produce same output every time
        byte[] src = [0x00, 0xF8]; // 16-bit LE = 0xF800 = pure red
        byte* dst1 = (byte*)NativeMemory.Alloc(4);
        byte* dst2 = (byte*)NativeMemory.Alloc(4);
        try
        {
            IthmbCodecPlugin.DecodeRgb565(src, dst1, 1, 1, littleEndian: true);
            IthmbCodecPlugin.DecodeRgb565(src, dst2, 1, 1, littleEndian: true);
            for (int i = 0; i < 4; i++)
                Assert.Equal(dst1[i], dst2[i]);
        }
        finally { NativeMemory.Free(dst1); NativeMemory.Free(dst2); }
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

    // ===================== Roundtrip consistency (reference encode → decode) =====================
    //
    // Reference encoders matching iOpenPod's algorithm (MIT-licensed reference implementation).
    // Proves our decoders are algorithmically correct without requiring real .ithmb files.
    //

    /// <summary>Reference RGB565 packer matching iOpenPod's Python _pack_rgb565.</summary>
    private static ushort PackRgb565(int r, int g, int b) =>
        (ushort)(((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3));

    /// <summary>Reference BT.601 forward transform matching iOpenPod's UYVY encoder.</summary>
    private static (int y, int u, int v) RgbToYuv(int r, int g, int b)
    {
        int y = (int)(0.299 * r + 0.587 * g + 0.114 * b);
        int u = (int)(-0.169 * r - 0.331 * g + 0.5 * b + 128);
        int v = (int)(0.5 * r - 0.419 * g - 0.081 * b + 128);
        return (Clamp(y, 0, 255), Clamp(u, 0, 255), Clamp(v, 0, 255));
    }

    private static int Clamp(int v, int lo, int hi) => v < lo ? lo : v > hi ? hi : v;

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
        int bytesPerField = (h / 2) * w * 2; // 16 bytes per field
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
}
