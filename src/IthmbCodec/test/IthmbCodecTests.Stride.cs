using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    // ===================== Input row stride =====================
    // Reuhno's iPod Classic 6G data proved that F1061 stores 56-pixel rows
    // (112 bytes) even though the declared width is 55. The decoder must
    // compute input stride from actual data size (src.Length / h), not
    // from declared width (w * 2). These tests verify that.

    [Fact]
    public void DecodeRgb565_PaddedRowStride_55x55_56pxRows()
    {
        // F1061-like: 55x55 nominal, 56-pixel rows, 6160 bytes total
        int nominalW = 55, nominalH = 55;
        int storedW = 56;
        int frameSize = storedW * nominalH * 2; // 6160

        var src = new byte[frameSize];
        for (int y = 0; y < nominalH; y++)
        {
            int rowBase = y * storedW * 2;
            for (int x = 0; x < nominalW; x++)
            {
                // RGB565 red pixel LE: 0x00, 0xF8
                src[rowBase + x * 2] = 0x00;
                src[rowBase + x * 2 + 1] = 0xF8;
            }
            // Fill padding pixel (column 55) with green to detect misalignment
            src[rowBase + nominalW * 2] = 0xE0;
            src[rowBase + nominalW * 2 + 1] = 0x07;
        }

        var dst = new byte[nominalW * nominalH * 4];
        fixed (byte* pDst = dst)
        {
            bool ok = IthmbCodecPlugin.DecodeRgb565(src, pDst, nominalW, nominalH,
                littleEndian: true);
            Assert.True(ok);
        }

        // Every decoded pixel should be red (BGRA: B=0, G=0, R=255, A=255)
        // If stride was computed as w*2=110 instead of 112, rows past row 0
        // would read from wrong source offsets and produce wrong colors.
        for (int i = 0; i < nominalW * nominalH; i++)
        {
            if (dst[i * 4 + 2] < 248)
            {
                int y = i / nominalW, x = i % nominalW;
                Assert.Fail($"Pixel ({x},{y}): B={dst[i*4]} G={dst[i*4+1]} R={dst[i*4+2]} A={dst[i*4+3]}");
            }
            Assert.InRange(dst[i * 4 + 2], 248, 255); // R (MSB replication)
        }
    }

    [Fact]
    public void DecodeRgb565_PaddedRowStride_SquareUnpadded_StillWorks()
    {
        // Verify that unpadded formats (w == stored_w) still decode correctly
        int w = 128, h = 128;
        int frameSize = w * h * 2; // 32768

        var src = new byte[frameSize];
        for (int y = 0; y < h; y++)
        {
            int rowBase = y * w * 2;
            for (int x = 0; x < w; x++)
            {
                // RGB565 green pixel LE: 0xE0, 0x07
                src[rowBase + x * 2] = 0xE0;
                src[rowBase + x * 2 + 1] = 0x07;
            }
        }

        var dst = new byte[w * h * 4];
        fixed (byte* pDst = dst)
        {
            bool ok = IthmbCodecPlugin.DecodeRgb565(src, pDst, w, h,
                littleEndian: true);
            Assert.True(ok);
        }

        for (int i = 0; i < w * h; i++)
        {
            Assert.Equal(0, dst[i * 4]);         // B
            Assert.InRange(dst[i * 4 + 1], 248, 255); // G
            Assert.Equal(0, dst[i * 4 + 2]);     // R
            Assert.Equal(255, dst[i * 4 + 3]);   // A
        }
    }

    [Fact]
    public void DecodeRgb565_PaddedRowStride_DifferentWidthHeight()
    {
        // Non-square padded: width < stored width
        int nominalW = 48, nominalH = 55;
        int storedW = 56;
        int frameSize = storedW * nominalH * 2;

        var src = new byte[frameSize];
        for (int y = 0; y < nominalH; y++)
        {
            int rowBase = y * storedW * 2;
            for (int x = 0; x < nominalW; x++)
            {
                src[rowBase + x * 2] = 0x00;
                src[rowBase + x * 2 + 1] = 0xF8; // red
            }
        }

        var dst = new byte[nominalW * nominalH * 4];
        fixed (byte* pDst = dst)
        {
            bool ok = IthmbCodecPlugin.DecodeRgb565(src, pDst, nominalW, nominalH,
                littleEndian: true);
            Assert.True(ok);
        }

        for (int i = 0; i < nominalW * nominalH; i++)
        {
            Assert.Equal(0, dst[i * 4]);
            Assert.Equal(0, dst[i * 4 + 1]);
            Assert.InRange(dst[i * 4 + 2], 248, 255);
            Assert.Equal(255, dst[i * 4 + 3]);
        }
    }
}
