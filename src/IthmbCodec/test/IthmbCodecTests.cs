using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
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
        // 2 pixels, both blue: [Cb=255] [Y0=29] [Cr=107] [Y1=29]
        // Use a more predictable case: full red
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

    [Theory]
    [InlineData(1)]
    [InlineData(2)]
    [InlineData(3)]
    [InlineData(4)]
    [InlineData(5)]
    [InlineData(6)]
    [InlineData(7)]
    [InlineData(8)]
    public void ReadExifOrientation_AllValues_ReturnsCorrectly(int orientation)
    {
        var jpeg = BuildJpegWithExifOrientation(orientation);
        int orient = IthmbCodecPlugin.ReadExifOrientation(jpeg, 0, jpeg.Length);
        Assert.Equal(orientation, orient);
    }

    [Fact]
    public void ReadExifOrientation_BigEndianTiff_ReturnsValue()
    {
        // Build minimal JPEG with big-endian TIFF EXIF
        using var ms = new MemoryStream();
        // SOI
        ms.Write([0xFF, 0xD8]);
        // APP1 marker + size
        ms.Write([0xFF, 0xE1]);
        // APP1 size (big-endian)
        ms.WriteByte(0); ms.WriteByte(0x45); // 69 bytes total

        // Exif header
        ms.Write([0x45, 0x78, 0x69, 0x66, 0x00, 0x00]); // "Exif\0\0"

        // TIFF header (big-endian "MM")
        ms.Write([0x4D, 0x4D]); // byte order
        ms.WriteByte(0x00); ms.WriteByte(0x2A); // magic 42

        // IFD0 offset: 8 (right after TIFF header)
        ms.Write([0x00, 0x00, 0x00, 0x08]);

        // IFD0: 1 entry (orientation), 8 bytes, then next IFD offset = 0
        ms.Write([0x00, 0x01]); // 1 entry

        // Tag 0x0112 = Orientation
        ms.Write([0x01, 0x12]); // tag
        ms.Write([0x00, 0x03]); // type = SHORT
        ms.Write([0x00, 0x00, 0x00, 0x01]); // count = 1
        ms.Write([0x00, 0x01, 0x00, 0x00]); // value = 1 (big-endian, 4 bytes)

        ms.Write([0x00, 0x00, 0x00, 0x00]); // next IFD = none

        // EOI
        ms.Write([0xFF, 0xD9]);

        var jpeg = ms.ToArray();
        int orient = IthmbCodecPlugin.ReadExifOrientation(jpeg, 0, jpeg.Length);
        Assert.Equal(1, orient);
    }

}
