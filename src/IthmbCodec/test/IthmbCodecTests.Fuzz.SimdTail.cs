// SPDX-License-Identifier: MIT
// SIMD tail path fuzz tests for non-power-of-2 widths and extreme dimensions

using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    [Fact]
    public void DecodeYcbcr420_Scalar_OddDimensions()
    {
        int w = 3, h = 3;
        int ySize = w * h;
        int uvSize = ((w + 1) / 2) * ((h + 1) / 2);
        var src = new byte[ySize + uvSize * 2];
        for (int i = 0; i < ySize; i++) src[i] = 128;
        for (int i = 0; i < uvSize; i++) src[ySize + i] = 128;
        for (int i = 0; i < uvSize; i++) src[ySize + uvSize + i] = 128;
        byte* dst = (byte*)NativeMemory.Alloc((nuint)(w * h * 4));
        try
        {
            IthmbCodecPlugin.DecodeYcbcr420(src, dst, w, h);
            for (int i = 0; i < w * h * 4; i += 4)
            {
                Assert.InRange(dst[i], 120, 136);
                Assert.InRange(dst[i + 1], 120, 136);
                Assert.InRange(dst[i + 2], 120, 136);
                Assert.Equal(255, dst[i + 3]);
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    private static IEnumerable<object[]> GetBuffers_SIMDTail(int seed)
    {
        var rng = new Random(seed);
        int[] sizes = [2, 4, 8, 16, 32, 64];
        for (int i = 0; i < 20; i++)
        {
            int w = sizes[rng.Next(sizes.Length)];
            int h = sizes[rng.Next(sizes.Length)];
            var buf = new byte[w * h * 2];
            rng.NextBytes(buf);
            yield return [buf, w, h];
        }
    }

    public static IEnumerable<object[]> GetBuffers_SIMDTail_Rgb565() => GetBuffers_SIMDTail(242);

    [Theory]
    [MemberData(nameof(GetBuffers_SIMDTail_Rgb565))]
    public void Fuzz_SIMDTail_RandomWidths(byte[] buf, int w, int h)
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
                Assert.InRange(dst[offset], 0, 255);
                Assert.InRange(dst[offset + 1], 0, 255);
                Assert.InRange(dst[offset + 2], 0, 255);
                Assert.Equal(255, dst[offset + 3]);
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    [Theory]
    [InlineData(2, 2)]
    [InlineData(4, 4)]
    [InlineData(8, 8)]
    [InlineData(16, 16)]
    [InlineData(32, 32)]
    [InlineData(64, 64)]
    public void Fuzz_SIMDTail_EdgeWidths(int w, int h)
    {
        var rng = new Random(99);
        var buf = new byte[w * h * 2];
        rng.NextBytes(buf);
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
                Assert.InRange(dst[offset], 0, 255);
                Assert.InRange(dst[offset + 1], 0, 255);
                Assert.InRange(dst[offset + 2], 0, 255);
                Assert.Equal(255, dst[offset + 3]);
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    [Theory]
    [InlineData(2, 8)]
    [InlineData(4, 16)]
    [InlineData(8, 2)]
    [InlineData(16, 4)]
    public void Fuzz_SIMDTail_NonSquare(int w, int h)
    {
        var rng = new Random(77);
        var buf = new byte[w * h * 2];
        rng.NextBytes(buf);
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
                Assert.InRange(dst[offset], 0, 255);
                Assert.InRange(dst[offset + 1], 0, 255);
                Assert.InRange(dst[offset + 2], 0, 255);
                Assert.Equal(255, dst[offset + 3]);
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    [Fact]
    public void Fuzz_SIMDTail_AllZeroInput()
    {
        int[] dims = [2, 4, 8, 16];
        foreach (int w in dims)
        {
            foreach (int h in dims)
            {
                var buf = new byte[w * h * 2];
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
                        Assert.InRange(dst[offset], 0, 255);
                        Assert.InRange(dst[offset + 1], 0, 255);
                        Assert.InRange(dst[offset + 2], 0, 255);
                        Assert.Equal(255, dst[offset + 3]);
                    }
                }
                finally { NativeMemory.Free(dst); }
            }
        }
    }

    [Fact]
    public void Fuzz_SIMDTail_EncodeDecode_Roundtrip()
    {
        int[] dims = [4, 8];
        var rng = new Random(123);
        foreach (int w in dims)
        {
            foreach (int h in dims)
            {
                var bgra = new byte[w * h * 4];
                for (int i = 0; i < w * h; i++)
                {
                    int r = rng.Next(256);
                    int g = rng.Next(256);
                    int b = rng.Next(256);
                    ushort packed = PackRgb565(r, g, b);
                    int r5 = (packed >> 11) & 0x1F;
                    int g6 = (packed >> 5) & 0x3F;
                    int b5 = packed & 0x1F;
                    bgra[i * 4] = (byte)((b5 << 3) | (b5 >> 2));
                    bgra[i * 4 + 1] = (byte)((g6 << 2) | (g6 >> 4));
                    bgra[i * 4 + 2] = (byte)((r5 << 3) | (r5 >> 2));
                    bgra[i * 4 + 3] = 255;
                }
                var profile = new IthmbCodecPlugin.IthmbVariantProfile(
                    Prefix: 9999, Width: w, Height: h,
                    Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
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
                    Assert.NotEqual((nint)0, (nint)outBuf->Data);
                    var decoded = new Span<byte>((void*)outBuf->Data, w * h * 4);
                    for (int i = 0; i < w * h; i++)
                    {
                        int px = i * 4;
                        Assert.Equal(bgra[px], decoded[px]);
                        Assert.Equal(bgra[px + 1], decoded[px + 1]);
                        Assert.Equal(bgra[px + 2], decoded[px + 2]);
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
    }
}
