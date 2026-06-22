using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    // ===================================================================
    // YUV422 Interlaced — must match non-interlaced for flat-color images
    // ===================================================================

    [Fact]
    public void Statistical_Interlaced_MatchesLinear_FlatColors()
    {
        foreach (int w in new[] { 2, 4, 8, 16, 32, 64 })
        {
            foreach (int h in new[] { 2, 4, 8, 16, 32, 64 })
            {
                if ((w & 1) != 0) continue;

                int totalBytes = w * h * 2;
                int halfBytes = ((h + 1) / 2) * w * 2;

                var rng = new Random(w * 137 + h);
                byte[] linearSrc = new byte[totalBytes];
                byte[] interlaceSrc = new byte[totalBytes];
                rng.NextBytes(linearSrc);

                for (int y = 0; y < h; y++)
                {
                    int fieldOffset = (y % 2 == 0) ? 0 : halfBytes;
                    int rowInField = y / 2;
                    int srcRow = y * w * 2;
                    int dstRow = fieldOffset + rowInField * w * 2;
                    Array.Copy(linearSrc, srcRow, interlaceSrc, dstRow, w * 2);
                }

                byte* linearDst = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
                byte* interlaceDst = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
                try
                {
                    NativeMemory.Clear(linearDst, (nuint)(w * h * 4));
                    NativeMemory.Clear(interlaceDst, (nuint)(w * h * 4));

                    IthmbCodecPlugin.DecodeYuv422(linearSrc, linearDst, w, h);
                    IthmbCodecPlugin.DecodeYuv422Interlaced(interlaceSrc, interlaceDst, w, h);

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
    }

    [Fact]
    public void Statistical_Interlaced_PixelRanges_Alpha()
    {
        var rng = new Random(4242);
        foreach (int w in new[] { 2, 4, 8, 16 })
        {
            foreach (int h in new[] { 2, 4, 8, 16 })
            {
                if ((w & 1) != 0) continue;
                int totalBytes = w * h * 2;
                byte[] src = new byte[totalBytes];
                rng.NextBytes(src);

                byte* dst = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
                try
                {
                    NativeMemory.Clear(dst, (nuint)(w * h * 4));
                    IthmbCodecPlugin.DecodeYuv422Interlaced(src, dst, w, h);

                    for (int i = 0; i < w * h; i++)
                    {
                        Assert.InRange(dst[i * 4], 0, 255);
                        Assert.InRange(dst[i * 4 + 1], 0, 255);
                        Assert.InRange(dst[i * 4 + 2], 0, 255);
                        Assert.Equal(255, dst[i * 4 + 3]);
                    }
                }
                finally { NativeMemory.Free(dst); }
            }
        }
    }

    [Fact]
    public void Statistical_Interlaced_FlatImageIdentity()
    {
        foreach (int w in new[] { 2, 4, 8, 16, 32 })
        {
            foreach (int h in new[] { 2, 4, 8, 16, 32 })
            {
                if ((w & 1) != 0) continue;
                int totalBytes = w * h * 2;
                int halfBytes = ((h + 1) / 2) * w * 2;
                var src = new byte[totalBytes];

                for (int y = 0; y < h; y++)
                {
                    int fieldOffset = (y % 2 == 0) ? 0 : halfBytes;
                    int rowInField = y / 2;
                    int off = fieldOffset + rowInField * w * 2;
                    for (int x = 0; x < w * 2; x++)
                        src[off + x] = 128;
                }

                byte* dst = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
                try
                {
                    IthmbCodecPlugin.DecodeYuv422Interlaced(src, dst, w, h);
                    for (int i = 0; i < w * h; i++)
                    {
                        Assert.InRange(dst[i * 4], 120, 136);
                        Assert.InRange(dst[i * 4 + 1], 120, 136);
                        Assert.InRange(dst[i * 4 + 2], 120, 136);
                        Assert.Equal(255, dst[i * 4 + 3]);
                    }
                }
                finally { NativeMemory.Free(dst); }
            }
        }
    }

    // ===================================================================
    // CLCL nibble-chroma
    // ===================================================================

    [Fact]
    public void Statistical_Clcl_Deterministic()
    {
        var rng = new Random(999);
        for (int trial = 0; trial < 100; trial++)
        {
            int w = FuzzSizes[rng.Next(FuzzSizes.Length)] & ~1;
            int h = FuzzSizes[rng.Next(FuzzSizes.Length)];
            if (w < 2 || h < 1) continue;
            int totalBytes = w * h * 2;
            var src = new byte[totalBytes];
            rng.NextBytes(src);

            byte* dst1 = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
            byte* dst2 = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
            try
            {
                NativeMemory.Clear(dst1, (nuint)(w * h * 4));
                NativeMemory.Clear(dst2, (nuint)(w * h * 4));

                IthmbCodecPlugin.DecodeYuv422Clcl(src, dst1, w, h);
                IthmbCodecPlugin.DecodeYuv422Clcl(src, dst2, w, h);

                for (int i = 0; i < w * h * 4; i++)
                    Assert.Equal(dst1[i], dst2[i]);
            }
            finally { NativeMemory.Free(dst1); NativeMemory.Free(dst2); }
        }
    }

    [Fact]
    public void Statistical_Clcl_ValidRanges()
    {
        var rng = new Random(4243);
        for (int trial = 0; trial < 200; trial++)
        {
            int w = FuzzSizes[rng.Next(FuzzSizes.Length)] & ~1;
            int h = FuzzSizes[rng.Next(FuzzSizes.Length)];
            if (w < 2 || h < 1) continue;
            int totalBytes = w * h * 2;
            var src = new byte[totalBytes];
            rng.NextBytes(src);

            byte* dst = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
            try
            {
                NativeMemory.Clear(dst, (nuint)(w * h * 4));
                IthmbCodecPlugin.DecodeYuv422Clcl(src, dst, w, h);

                for (int i = 0; i < w * h; i++)
                {
                    Assert.InRange(dst[i * 4], 0, 255);
                    Assert.InRange(dst[i * 4 + 1], 0, 255);
                    Assert.InRange(dst[i * 4 + 2], 0, 255);
                    Assert.Equal(255, dst[i * 4 + 3]);
                }
            }
            finally { NativeMemory.Free(dst); }
        }
    }

    [Fact]
    public void Statistical_Clcl_NeutralChromaRoundtrip()
    {
        for (int w = 2; w <= 32; w += 2)
        {
            for (int h = 1; h <= 16; h++)
            {
                int pixelCount = w * h;
                var bgra = new byte[pixelCount * 4];
                for (int i = 0; i < pixelCount; i++)
                {
                    int v = (i * 17) & 0xFF;
                    bgra[i * 4] = (byte)v;
                    bgra[i * 4 + 1] = (byte)v;
                    bgra[i * 4 + 2] = (byte)v;
                    bgra[i * 4 + 3] = 255;
                }

                var profile = new IthmbCodecPlugin.IthmbVariantProfile(
                    9998, w, h, IthmbCodecPlugin.IthmbEncoding.Yuv422,
                    w * h * 2, ClclChroma: true);

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

                    var decoded = new Span<byte>((void*)outBuf->Data, pixelCount * 4);
                    for (int i = 0; i < pixelCount; i++)
                    {
                        Assert.InRange(decoded[i * 4], 0, 255);
                        Assert.InRange(decoded[i * 4 + 1], 0, 255);
                        Assert.InRange(decoded[i * 4 + 2], 0, 255);
                        Assert.Equal(255, decoded[i * 4 + 3]);
                    }
                }
                finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
            }
        }
    }
}
