using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
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

    private static unsafe bool MemCmp(byte* a, byte* b, int len)
    {
        for (int i = 0; i < len; i++)
            if (a[i] != b[i]) return false;
        return true;
    }

    /// <summary>Reference RGB565 packer matching iOpenPod's Python _pack_rgb565.</summary>
    private static ushort PackRgb565(int r, int g, int b) =>
        (ushort)(((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3));

    private static (int y, int u, int v) RgbToYuv(int r, int g, int b)
    {
        int y = (77 * r + 150 * g + 29 * b) >> 8;
        int u = ((-43 * r - 85 * g + 128 * b) >> 8) + 128;
        int v = ((128 * r - 107 * g - 21 * b) >> 8) + 128;
        return (Clamp(y, 0, 255), Clamp(u, 0, 255), Clamp(v, 0, 255));
    }

    private static int Clamp(int v, int lo, int hi) => v < lo ? lo : v > hi ? hi : v;

    private static int Clamp(int v) => Clamp(v, 0, 255);

    /// <summary>Shared determinism helper: decode twice into separate buffers, compare byte-for-byte.</summary>
    /// <remarks>Uses <c>IntPtr</c> instead of <c>byte*</c> because C# forbids pointer types as generic arguments.</remarks>
    private static void AssertDeterminism(int dstByteCount, Func<IntPtr, bool> decode)
    {
        byte* dst1 = (byte*)NativeMemory.Alloc((nuint)dstByteCount);
        byte* dst2 = (byte*)NativeMemory.Alloc((nuint)dstByteCount);
        try
        {
            NativeMemory.Clear(dst1, (nuint)dstByteCount);
            NativeMemory.Clear(dst2, (nuint)dstByteCount);
            Assert.True(decode((IntPtr)dst1));
            Assert.True(decode((IntPtr)dst2));
            Assert.True(MemCmp(dst1, dst2, dstByteCount));
        }
        finally
        {
            NativeMemory.Free(dst1);
            NativeMemory.Free(dst2);
        }
    }

    /// <summary>Randomly mutates a byte buffer for corruption fuzz testing.</summary>
    private static void MutateBuffer(Random rng, byte[] buf)
    {
        double roll = rng.NextDouble();
        if (roll < 0.10)
        {
            int pos = rng.Next(buf.Length);
            int bit = rng.Next(8);
            buf[pos] ^= (byte)(1 << bit);
        }
        else if (roll < 0.15)
        {
            int a = rng.Next(buf.Length);
            int b = rng.Next(buf.Length);
            (buf[a], buf[b]) = (buf[b], buf[a]);
        }
        else if (roll < 0.20)
        {
            int newLen = rng.Next(4, buf.Length + 1);
            Array.Resize(ref buf, newLen);
        }
    }

    /// <summary>Asserts that a pixel buffer has valid channel ranges and alpha=255 for every pixel.</summary>
    private static void AssertValidPixels(byte* dst, int pixelCount)
    {
        for (int i = 0; i < pixelCount; i++)
        {
            int off = i * 4;
            Assert.InRange(dst[off], 0, 255);     // B
            Assert.InRange(dst[off + 1], 0, 255); // G
            Assert.InRange(dst[off + 2], 0, 255); // R
            Assert.Equal(255, dst[off + 3]);         // A
        }
    }
}
