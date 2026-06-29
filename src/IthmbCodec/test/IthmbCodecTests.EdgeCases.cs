// Edge-case coverage: null dst, zero dimensions, minimum-size buffers
using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    // ---- Zero dimensions ----

    [Fact]
    public void DecodeRgb565_ZeroWidth_ReturnsFalse()
    {
        var buf = new byte[16];
        byte* dst = (byte*)NativeMemory.Alloc(16);
        try
        {
            Assert.False(IthmbCodecPlugin.DecodeRgb565(buf, dst, 0, 4, littleEndian: true));
            Assert.False(IthmbCodecPlugin.DecodeRgb565(buf, dst, 4, 0, littleEndian: true));
            Assert.False(IthmbCodecPlugin.DecodeRgb565(buf, dst, 0, 0, littleEndian: true));
        }
        finally { NativeMemory.Free(dst); }
    }

    [Fact]
    public void DecodeRgb555_ZeroWidth_ReturnsFalse()
    {
        var buf = new byte[16];
        byte* dst = (byte*)NativeMemory.Alloc(16);
        try
        {
            Assert.False(IthmbCodecPlugin.DecodeRgb555(buf, dst, 0, 4, littleEndian: true));
            Assert.False(IthmbCodecPlugin.DecodeRgb555(buf, dst, 4, 0, littleEndian: true));
            Assert.False(IthmbCodecPlugin.DecodeRgb555(buf, dst, 0, 0, littleEndian: true));
        }
        finally { NativeMemory.Free(dst); }
    }

    [Fact]
    public void DecodeYuv422_ZeroWidth_ReturnsFalse()
    {
        var buf = new byte[16];
        byte* dst = (byte*)NativeMemory.Alloc(16);
        try
        {
            Assert.False(IthmbCodecPlugin.DecodeYuv422(buf, dst, 0, 4));
            Assert.False(IthmbCodecPlugin.DecodeYuv422(buf, dst, 4, 0));
            Assert.False(IthmbCodecPlugin.DecodeYuv422(buf, dst, 0, 0));
        }
        finally { NativeMemory.Free(dst); }
    }

    [Fact]
    public void DecodeYuv422Interlaced_ZeroWidth_ReturnsFalse()
    {
        var buf = new byte[16];
        byte* dst = (byte*)NativeMemory.Alloc(16);
        try
        {
            Assert.False(IthmbCodecPlugin.DecodeYuv422Interlaced(buf, dst, 0, 4));
            Assert.False(IthmbCodecPlugin.DecodeYuv422Interlaced(buf, dst, 4, 0));
            Assert.False(IthmbCodecPlugin.DecodeYuv422Interlaced(buf, dst, 0, 0));
        }
        finally { NativeMemory.Free(dst); }
    }

    [Fact]
    public void DecodeYcbcr420_ZeroWidth_ReturnsFalse()
    {
        var buf = new byte[16];
        byte* dst = (byte*)NativeMemory.Alloc(16);
        try
        {
            Assert.False(IthmbCodecPlugin.DecodeYcbcr420(buf, dst, 0, 4));
            Assert.False(IthmbCodecPlugin.DecodeYcbcr420(buf, dst, 4, 0));
            Assert.False(IthmbCodecPlugin.DecodeYcbcr420(buf, dst, 0, 0));
        }
        finally { NativeMemory.Free(dst); }
    }

    [Fact]
    public void DecodeYuv422Clcl_ZeroWidth_ReturnsFalse()
    {
        var buf = new byte[16];
        byte* dst = (byte*)NativeMemory.Alloc(16);
        try
        {
            Assert.False(IthmbCodecPlugin.DecodeYuv422Clcl(buf, dst, 0, 4));
            Assert.False(IthmbCodecPlugin.DecodeYuv422Clcl(buf, dst, 4, 0));
            Assert.False(IthmbCodecPlugin.DecodeYuv422Clcl(buf, dst, 0, 0));
        }
        finally { NativeMemory.Free(dst); }
    }

    [Fact]
    public void DecodeYuv422Cl_ZeroWidth_ReturnsFalse()
    {
        var buf = new byte[16];
        byte* dst = (byte*)NativeMemory.Alloc(16);
        try
        {
            Assert.False(IthmbCodecPlugin.DecodeYuv422Cl(buf, dst, 0, 4));
            Assert.False(IthmbCodecPlugin.DecodeYuv422Cl(buf, dst, 4, 0));
            Assert.False(IthmbCodecPlugin.DecodeYuv422Cl(buf, dst, 0, 0));
        }
        finally { NativeMemory.Free(dst); }
    }

    // ---- null pointer safety via DecodeRawProfile ----
    // Direct null* dst calls to raw decoders would segfault.
    // DecodeRawProfile allocates its own buffer and sets outBuf->Data, so
    // null outBuf->Data is harmless. null outBuf itself is handled at line 279.

    [Fact]
    public void DecodeRawProfile_NullOutBuf_ReturnsOk()
    {
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 10000, Width: 1, Height: 1,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: 2);

        byte[] bgra = [0, 0, 255, 255];
        byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, 1, 1, profile);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        try
        {
            // outBuf = null: should fill image info and return OK
            var status = IthmbCodecPlugin.DecodeRawProfile(ithmbFile, profile,
                cancellation: null, outInfo, null);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
        }
        finally
        {
            NativeMemory.Free(outInfo);
        }
    }

}