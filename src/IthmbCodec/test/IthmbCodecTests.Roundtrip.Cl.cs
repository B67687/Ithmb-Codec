using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

// SIZE_OK: CL roundtrip + multi-frame tests (~350 pure LOC)
public unsafe partial class IthmbCodecTests
{
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
    public void RotateBgra_90_Correctness()
    {
        // Direct RotateBgra test: 2×4 image, 90° CW → 4×2
        // marker index = x*srcH + (srcH-1-y) = 0*4+3 = 3 (byte offset 12)
        int w = 2, h = 4;
        byte[] bgra = new byte[w * h * 4];
        bgra[0] = 255; bgra[1] = 0; bgra[2] = 0; bgra[3] = 255; // blue at (0,0)
        for (int i = 1; i < w * h; i++)
        {
            bgra[i * 4]     = 0;     // all others: red (B=0, G=0, R=255)
            bgra[i * 4 + 1] = 0;
            bgra[i * 4 + 2] = 255;
            bgra[i * 4 + 3] = 255;
        }

        unsafe
        {
            fixed (byte* p = bgra)
            {
                int rw = w, rh = h;
                IthmbCodecPlugin.RotateBgra(p, ref rw, ref rh, 90);
                Assert.Equal(4, rw);
                Assert.Equal(2, rh);
                // Blue marker moved to position 3 (byte 12-15)
                Assert.Equal(255, p[12]); Assert.Equal(0, p[13]);
                Assert.Equal(0,   p[14]); Assert.Equal(255, p[15]);
                // Position 0 = source (0,3) which was red (byte 0-3)
                Assert.Equal(0,   p[0]);  Assert.Equal(0, p[1]);
                Assert.Equal(255, p[2]);  Assert.Equal(255, p[3]);
            }
        }
    }

    [Fact]
    public void RotateBgra_270_Correctness()
    {
        // 4×2 image, 270° CW → 2×4
        // marker index = (srcW-1-x)*srcH + y = (3-0)*2+0 = 6 (byte offset 24)
        int w = 4, h = 2;
        byte[] bgra = new byte[w * h * 4];
        bgra[0] = 255; bgra[1] = 0; bgra[2] = 0; bgra[3] = 255; // blue at (0,0)
        for (int i = 1; i < w * h; i++)
        {
            bgra[i * 4]     = 0;
            bgra[i * 4 + 1] = 0;
            bgra[i * 4 + 2] = 255;
            bgra[i * 4 + 3] = 255;
        }

        unsafe
        {
            fixed (byte* p = bgra)
            {
                int rw = w, rh = h;
                IthmbCodecPlugin.RotateBgra(p, ref rw, ref rh, 270);
                Assert.Equal(2, rw);
                Assert.Equal(4, rh);
                // Blue marker at position 6 (byte 24-27)
                Assert.Equal(255, p[24]); Assert.Equal(0, p[25]);
                Assert.Equal(0,   p[26]); Assert.Equal(255, p[27]);
            }
        }
    }

    [Fact]
    public void RotateBgra_90_Roundtrip_Identity()
    {
        // Roundtrip without rotation: BuildIthmbFile populates pixels then
        // DecodeRawProfile decodes them. Uses Rotation=0 so no pre-rotation.
        int w = 2, h = 4;
        var bgra = new byte[w * h * 4];
        for (int i = 0; i < w * h; i++)
        {
            bgra[i * 4]     = (byte)(i * 5);
            bgra[i * 4 + 1] = (byte)(i * 9);
            bgra[i * 4 + 2] = (byte)(i * 13);
            bgra[i * 4 + 3] = 255;
        }

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 9997, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: w * h * 2, Rotation: 0);

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
            Assert.Equal(w, outBuf->Width);
            Assert.Equal(h, outBuf->Height);

            var decoded = new Span<byte>((void*)outBuf->Data, w * h * 4);
            for (int i = 0; i < w * h * 4; i++)
                Assert.InRange(decoded[i], bgra[i] - 8, bgra[i] + 8);
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
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.InvalidArg, status);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    [Fact]
    public void MultiFrame_RawDecode_NegativeIndex_ReturnsInvalidArg()
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
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.InvalidArg, status);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }
}
