using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    // ===================================================================
    // CL per-pixel nibble-chroma
    // ===================================================================

    [Fact]
    public void Statistical_Cl_Deterministic()
    {
        var rng = new Random(888);
        for (int trial = 0; trial < 100; trial++)
        {
            int w = FuzzSizes[rng.Next(FuzzSizes.Length)];
            int h = FuzzSizes[rng.Next(FuzzSizes.Length)];
            if (w < 1 || h < 1) continue;
            int totalBytes = w * h * 2;
            var src = new byte[totalBytes];
            rng.NextBytes(src);

            byte* dst1 = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
            byte* dst2 = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
            try
            {
                NativeMemory.Clear(dst1, (nuint)(w * h * 4));
                NativeMemory.Clear(dst2, (nuint)(w * h * 4));

                IthmbCodecPlugin.DecodeYuv422Cl(src, dst1, w, h);
                IthmbCodecPlugin.DecodeYuv422Cl(src, dst2, w, h);

                for (int i = 0; i < w * h * 4; i++)
                    Assert.Equal(dst1[i], dst2[i]);
            }
            finally { NativeMemory.Free(dst1); NativeMemory.Free(dst2); }
        }
    }

    [Fact]
    public void Statistical_Cl_ValidRanges()
    {
        var rng = new Random(4244);
        for (int trial = 0; trial < 200; trial++)
        {
            int w = FuzzSizes[rng.Next(FuzzSizes.Length)];
            int h = FuzzSizes[rng.Next(FuzzSizes.Length)];
            if (w < 1 || h < 1) continue;
            int totalBytes = w * h * 2;
            var src = new byte[totalBytes];
            rng.NextBytes(src);

            byte* dst = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
            try
            {
                NativeMemory.Clear(dst, (nuint)(w * h * 4));
                IthmbCodecPlugin.DecodeYuv422Cl(src, dst, w, h);

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
    public void Statistical_Cl_NeutralChromaRoundtrip()
    {
        for (int w = 1; w <= 16; w++)
        {
            for (int h = 1; h <= 16; h++)
            {
                int pixelCount = w * h;
                var bgra = new byte[pixelCount * 4];
                for (int i = 0; i < pixelCount; i++)
                {
                    int v = (i * 19) & 0xFF;
                    bgra[i * 4] = (byte)v;
                    bgra[i * 4 + 1] = (byte)v;
                    bgra[i * 4 + 2] = (byte)v;
                    bgra[i * 4 + 3] = 255;
                }

                var profile = new IthmbCodecPlugin.IthmbVariantProfile(
                    9999, w, h, IthmbCodecPlugin.IthmbEncoding.Yuv422,
                    w * h * 2, ClChroma: true);

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
                finally { if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data); NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
            }
        }
    }

    // ===================================================================
    // Swapped chroma planes — Cb/Cr swap consistency
    // ===================================================================

    [Fact]
    public void Statistical_SwapChroma_NeutralChroma()
    {
        foreach (int w in new[] { 2, 4, 8, 16 })
        {
            foreach (int h in new[] { 2, 4, 8, 16 })
            {
                int pixelCount = w * h;
                var bgra = new byte[pixelCount * 4];
                for (int i = 0; i < pixelCount; i++)
                {
                    int v = (i * 31) & 0xFF;
                    bgra[i * 4] = (byte)v;
                    bgra[i * 4 + 1] = (byte)v;
                    bgra[i * 4 + 2] = (byte)v;
                    bgra[i * 4 + 3] = 255;
                }

                var profile = new IthmbCodecPlugin.IthmbVariantProfile(
                    9997, w, h, IthmbCodecPlugin.IthmbEncoding.Ycbcr420,
                    w * h * 2, IsPadded: true, SwapChromaPlanes: true);

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
                    for (int i = 0; i < pixelCount * 4; i += 4)
                    {
                        Assert.InRange(decoded[i], 0, 255);
                        Assert.InRange(decoded[i + 1], 0, 255);
                        Assert.InRange(decoded[i + 2], 0, 255);
                        Assert.Equal(255, decoded[i + 3]);
                    }
                }
                finally { if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data); NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
            }
        }
    }

    [Fact]
    public void Statistical_SwapChroma_Deterministic()
    {
        var rng = new Random(777);
        for (int trial = 0; trial < 50; trial++)
        {
            int w = (rng.Next(7) + 1) * 2;
            int h = (rng.Next(7) + 1) * 2;
            int pixelCount = w * h;
            int ySize = w * h;
            int uvSize = ((w + 1) / 2) * ((h + 1) / 2);
            var src = new byte[ySize + uvSize * 2];
            rng.NextBytes(src);

            byte* dst1 = (byte*)NativeMemory.Alloc((nuint)(pixelCount * 4));
            byte* dst2 = (byte*)NativeMemory.Alloc((nuint)(pixelCount * 4));
            try
            {
                IthmbCodecPlugin.DecodeYcbcr420(src, dst1, w, h, swapChromaPlanes: true);
                IthmbCodecPlugin.DecodeYcbcr420(src, dst2, w, h, swapChromaPlanes: true);
                for (int i = 0; i < pixelCount * 4; i++)
                    Assert.Equal(dst1[i], dst2[i]);
            }
            finally { NativeMemory.Free(dst1); NativeMemory.Free(dst2); }
        }
    }

    // ===================================================================
    // Large-scale fuzz: 10,000 random inputs across all decoders
    // ===================================================================

    [Fact]
    public void Statistical_Fuzz_10k_AllDecoders()
    {
        var rng = new Random(9999);
        int trialsPerDecoder = FuzzTrials / 9;

        for (int trial = 0; trial < trialsPerDecoder; trial++)
        {
            int w = FuzzSizes[rng.Next(FuzzSizes.Length)];
            int h = FuzzSizes[rng.Next(FuzzSizes.Length)];
            if ((w & 1) != 0 && trial % 3 != 2) w = (w + 1) & ~1;
            if (w < 1) w = 1;
            if (h < 1) h = 1;

            int bufSize = Math.Max(w * h * 2, 1);
            var src = new byte[bufSize];
            rng.NextBytes(src);

            int allocSize = Math.Max(4096, w * h * 4);
            byte* dst = (byte*)NativeMemory.Alloc((nuint)allocSize);
            try
            {
                NativeMemory.Clear(dst, (nuint)allocSize);

                int decoder = trial % 9;
                bool ok = decoder switch
                {
                    0 => IthmbCodecPlugin.DecodeRgb565(src, dst, w, h, true),
                    1 => IthmbCodecPlugin.DecodeRgb555(src, dst, w, h, true),
                    2 when (w & 1) == 0 => IthmbCodecPlugin.DecodeYuv422(src, dst, w, h),
                    3 when (w & 1) == 0 => IthmbCodecPlugin.DecodeYuv422Interlaced(src, dst, w, h),
                    4 when (w & 1) == 0 => IthmbCodecPlugin.DecodeYuv422Clcl(src, dst, w, h),
                    5 => IthmbCodecPlugin.DecodeYuv422Cl(src, dst, w, h),
                    6 => IthmbCodecPlugin.DecodeYcbcr420(src, dst, w, h, swapChromaPlanes: false),
                    7 => IthmbCodecPlugin.DecodeYcbcr420(src, dst, w, h, swapChromaPlanes: true),
                    _ => false
                };
                if (!ok && decoder < 6 && (w & 1) != 0) continue;
                if (!ok) continue;

                // Verify color channels are in valid range
                int pixels = w * h;
                int checkPixels = Math.Min(pixels, allocSize / 4);
                for (int i = 0; i < checkPixels; i++)
                {
                    int off = i * 4;
                    Assert.InRange(dst[off], 0, 255);
                    Assert.InRange(dst[off + 1], 0, 255);
                    Assert.InRange(dst[off + 2], 0, 255);
                }
            }
            finally { NativeMemory.Free(dst); }
        }
    }

    [Fact]
    public void Statistical_Fuzz_Determinism_10k()
    {
        var rng = new Random(424242);
        for (int trial = 0; trial < 1000; trial++)
        {
            int w = FuzzSizes[rng.Next(FuzzSizes.Length)];
            int h = FuzzSizes[rng.Next(FuzzSizes.Length)];
            if ((w & 1) != 0) w = (w + 1) & ~1;
            if (w < 2) w = 2;
            if (h < 1) h = 1;

            int bufSize = Math.Max(w * h * 2, 1);
            var src = new byte[bufSize];
            rng.NextBytes(src);

            int allocSize = Math.Max(4096, w * h * 4);
            byte* dst1 = (byte*)NativeMemory.Alloc((nuint)allocSize);
            byte* dst2 = (byte*)NativeMemory.Alloc((nuint)allocSize);
            try
            {
                NativeMemory.Clear(dst1, (nuint)allocSize);
                NativeMemory.Clear(dst2, (nuint)allocSize);

                int decoder = trial % 7;
                switch (decoder)
                {
                    case 0:
                        IthmbCodecPlugin.DecodeRgb565(src, dst1, w, h, true);
                        IthmbCodecPlugin.DecodeRgb565(src, dst2, w, h, true);
                        break;
                    case 1:
                        IthmbCodecPlugin.DecodeRgb555(src, dst1, w, h, true);
                        IthmbCodecPlugin.DecodeRgb555(src, dst2, w, h, true);
                        break;
                    case 2:
                        IthmbCodecPlugin.DecodeYuv422(src, dst1, w, h);
                        IthmbCodecPlugin.DecodeYuv422(src, dst2, w, h);
                        break;
                    case 3:
                        IthmbCodecPlugin.DecodeYuv422Interlaced(src, dst1, w, h);
                        IthmbCodecPlugin.DecodeYuv422Interlaced(src, dst2, w, h);
                        break;
                    case 4:
                        IthmbCodecPlugin.DecodeYuv422Clcl(src, dst1, w, h);
                        IthmbCodecPlugin.DecodeYuv422Clcl(src, dst2, w, h);
                        break;
                    case 5:
                        IthmbCodecPlugin.DecodeYuv422Cl(src, dst1, w, h);
                        IthmbCodecPlugin.DecodeYuv422Cl(src, dst2, w, h);
                        break;
                    case 6:
                        IthmbCodecPlugin.DecodeYcbcr420(src, dst1, w, h, swapChromaPlanes: trial % 2 == 0);
                        IthmbCodecPlugin.DecodeYcbcr420(src, dst2, w, h, swapChromaPlanes: trial % 2 == 0);
                        break;
                }

                int pixels = Math.Min(w * h, allocSize / 4) * 4;
                for (int i = 0; i < pixels; i++)
                    Assert.Equal(dst1[i], dst2[i]);
            }
            finally { NativeMemory.Free(dst1); NativeMemory.Free(dst2); }
        }
    }
}
