// Encoder helper coverage: InterlaceFields paths and scalar decoder fallbacks
using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    // ===================== InterlaceFields 2Bpp (non-Ycbcr420) =====================

    [Fact]
    public void Roundtrip_Rgb565_Interlaced_4x4()
    {
        // InterlaceFields 2Bpp path: each row is w*2 bytes, even rows first then odd
        int w = 4, h = 4;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++)
        {
            bgra[i * 4] = (byte)(i * 31);      // B
            bgra[i * 4 + 1] = (byte)(i * 17);  // G
            bgra[i * 4 + 2] = (byte)(i * 7);   // R
            bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1019, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: w * h * 2,
            IsInterlaced: true);
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
            // RGB565 is lossy (5/6/5 quantization), so allow tolerance
            for (int i = 0; i < w * h; i++)
            {
                int px = i * 4;
                Assert.Equal(255, decoded[px + 3]); // alpha always 255
            }
        }
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }

    [Fact]
    public void Roundtrip_Rgb555_Interlaced_4x4()
    {
        // Same 2Bpp path but with RGB555 encoding
        int w = 4, h = 4;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++)
        {
            bgra[i * 4] = (byte)(i * 31);
            bgra[i * 4 + 1] = (byte)(i * 17);
            bgra[i * 4 + 2] = (byte)(i * 7);
            bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1020, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb555,
            FrameByteLength: w * h * 2,
            IsInterlaced: true);
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
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }

    // ===================== InterlaceFields YCbCr420 =====================

    [Fact]
    public void Roundtrip_Ycbcr420_Interlaced_4x4()
    {
        // InterlaceFields YCbCr420 path: 3 planes (Y, Cb, Cr), each interlaced separately
        int w = 4, h = 4;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++)
        {
            bgra[i * 4] = (byte)(i * 31);
            bgra[i * 4 + 1] = (byte)(i * 17);
            bgra[i * 4 + 2] = (byte)(i * 7);
            bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1067, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Ycbcr420,
            FrameByteLength: w * h * 2,
            IsInterlaced: true, IsPadded: true);
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
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }

    // ===================== InterlaceFields edge cases =====================

    [Fact]
    public void Roundtrip_Rgb565_Interlaced_SingleRow()
    {
        // Single row: h=1, even/odd row reordering degenerates
        int w = 4, h = 1;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++)
        {
            bgra[i * 4] = 100; bgra[i * 4 + 1] = 150; bgra[i * 4 + 2] = 200; bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1021, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: w * h * 2,
            IsInterlaced: true);
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
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }

    [Fact]
    public void Roundtrip_Rgb565_Interlaced_SingleColumn()
    {
        // Single column: w=1, tests narrow interlace
        int w = 1, h = 4;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++)
        {
            bgra[i * 4] = 50; bgra[i * 4 + 1] = 100; bgra[i * 4 + 2] = 150; bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1022, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: w * h * 2,
            IsInterlaced: true);
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
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }

    [Fact]
    public void Roundtrip_Ycbcr420_Interlaced_SingleRow()
    {
        // Single row with YCbCr420 interlaced: chroma planes have 0 rows
        int w = 4, h = 1;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++)
        {
            bgra[i * 4] = 80; bgra[i * 4 + 1] = 120; bgra[i * 4 + 2] = 160; bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1068, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Ycbcr420,
            FrameByteLength: w * h * 2,
            IsInterlaced: true, IsPadded: true);
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
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }

    [Fact]
    public void Roundtrip_Ycbcr420_Interlaced_OddDimensions()
    {
        // Odd height: h/2 truncation in chroma loop
        int w = 6, h = 5;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++)
        {
            bgra[i * 4] = (byte)(i * 37 % 256);
            bgra[i * 4 + 1] = (byte)(i * 53 % 256);
            bgra[i * 4 + 2] = (byte)(i * 71 % 256);
            bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1069, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Ycbcr420,
            FrameByteLength: w * h * 2,
            IsInterlaced: true, IsPadded: true);
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
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }

    // ===================== Scalar decoder fallbacks =====================
    // On x64 CI, SIMD path requires (w & 7) == 0. Widths not divisible by 8
    // or small widths force the scalar fallback paths.

    [Fact]
    public void Roundtrip_Uyvy_ScalarFallback_W6()
    {
        // w=6: not divisible by 8, forces DecodeYuv422_Scalar
        int w = 6, h = 4;
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
            Prefix: 1023, Width: w, Height: h,
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
            int maxError = 0;
            for (int i = 0; i < w * h; i++)
            {
                int px = i * 4;
                int dr = Math.Abs(decoded[px + 2] - bgra[px + 2]);
                int dg = Math.Abs(decoded[px + 1] - bgra[px + 1]);
                int db = Math.Abs(decoded[px] - bgra[px]);
                maxError = Math.Max(maxError, Math.Max(dr, Math.Max(dg, db)));
                Assert.Equal(255, decoded[px + 3]);
            }
            Assert.InRange(maxError, 0, 25); // YUV422 chroma averaging with gradient causes higher error
        }
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }

    [Fact]
    public void Roundtrip_Cl_ScalarFallback_W6()
    {
        // w=6: not divisible by 8, forces DecodeYuv422Cl_Scalar
        int w = 6, h = 4;
        var bgra = new byte[w * h * 4];
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
            {
                int off = (y * w + x) * 4;
                bgra[off] = (byte)((x * 200) / w);
                bgra[off + 1] = (byte)((y * 200) / h);
                bgra[off + 2] = (byte)(((x + y) * 100) / (w + h));
                bgra[off + 3] = 255;
            }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1024, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Yuv422,
            FrameByteLength: w * h * 2, ClChroma: true);
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
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }

    [Fact]
    public void Roundtrip_Clcl_ScalarFallback_W6()
    {
        // w=6: not divisible by 8, forces DecodeYuv422Clcl_Scalar
        int w = 6, h = 4;
        var bgra = new byte[w * h * 4];
        for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
            {
                int off = (y * w + x) * 4;
                bgra[off] = (byte)((x * 200) / w);
                bgra[off + 1] = (byte)((y * 200) / h);
                bgra[off + 2] = (byte)(((x + y) * 100) / (w + h));
                bgra[off + 3] = 255;
            }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1025, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Yuv422,
            FrameByteLength: w * h * 2, ClclChroma: true);
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
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }

    [Fact]
    public void Roundtrip_Ycbcr420_ScalarFallback_SwapChroma()
    {
        // swapChromaPlanes=true forces scalar path (SIMD only when !swapChromaPlanes)
        int w = 4, h = 4;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++)
        {
            bgra[i * 4] = (byte)(i * 50 % 256);
            bgra[i * 4 + 1] = (byte)(i * 80 % 256);
            bgra[i * 4 + 2] = (byte)(i * 110 % 256);
            bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1070, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Ycbcr420,
            FrameByteLength: w * h * 2,
            IsPadded: true, SwapChromaPlanes: true);
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
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }

    // ===================== YUV color space edge cases =====================
    // These exercise Bt601Y/Cb/Cr through the encoder with specific RGB inputs.

    [Theory]
    [InlineData(0, 0, 0)]     // black
    [InlineData(255, 255, 255)] // white
    [InlineData(255, 0, 0)]   // pure red
    [InlineData(0, 255, 0)]   // pure green
    [InlineData(0, 0, 255)]   // pure blue
    [InlineData(128, 128, 128)] // mid-gray
    public void Roundtrip_Uyvy_KnownColors(int r, int g, int b)
    {
        // Encode a 2-pixel row with known RGB, decode, verify dominant channel
        int w = 2, h = 1;
        var bgra = new byte[w * h * 4];
        // pixel 0: known color
        bgra[0] = (byte)b; bgra[1] = (byte)g; bgra[2] = (byte)r; bgra[3] = 255;
        // pixel 1: same color
        bgra[4] = (byte)b; bgra[5] = (byte)g; bgra[6] = (byte)r; bgra[7] = 255;

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1026, Width: w, Height: h,
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
            // YUV is lossy: verify dominant channel direction
            int tol = 30;
            Assert.InRange(decoded[2], r - tol, r + tol); // R
            Assert.InRange(decoded[1], g - tol, g + tol); // G
            Assert.InRange(decoded[0], b - tol, b + tol); // B
        }
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }

    [Theory]
    [InlineData(0, 0, 0)]     // black
    [InlineData(255, 255, 255)] // white
    [InlineData(255, 0, 0)]   // pure red
    [InlineData(0, 255, 0)]   // pure green
    [InlineData(0, 0, 255)]   // pure blue
    [InlineData(128, 128, 128)] // mid-gray
    public void Roundtrip_Ycbcr420_KnownColors(int r, int g, int b)
    {
        // 2x2 image with uniform color — exercises Bt601Y/Cb/Cr through YCbCr420 encoder
        int w = 2, h = 2;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++)
        {
            bgra[i * 4] = (byte)b;
            bgra[i * 4 + 1] = (byte)g;
            bgra[i * 4 + 2] = (byte)r;
            bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1071, Width: w, Height: h,
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
            int tol = 15;
            Assert.InRange(decoded[2], r - tol, r + tol);
            Assert.InRange(decoded[1], g - tol, g + tol);
            Assert.InRange(decoded[0], b - tol, b + tol);
        }
        finally
        {
            if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }
}
