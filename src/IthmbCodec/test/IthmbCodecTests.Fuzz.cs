using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    // ---- Test 3: Fuzz — random buffers (no crash, valid output ranges) ----

    private static IEnumerable<object[]> GetBuffers(int seed)
    {
        var rng = new Random(seed);
        int[] sizes = [2, 4, 8, 16, 32, 64, 128];
        for (int i = 0; i < 50; i++)
        {
            int w = sizes[rng.Next(sizes.Length)];
            int h = sizes[rng.Next(sizes.Length)];
            var buf = new byte[w * h * 2];
            rng.NextBytes(buf);
            yield return [buf, w, h];
        }
    }

    public static IEnumerable<object[]> GetBuffers_Rgb565() => GetBuffers(42);
    public static IEnumerable<object[]> GetBuffers_Yuv422() => GetBuffers(43);
    public static IEnumerable<object[]> GetBuffers_Ycbcr420() => GetBuffers(44);
    public static IEnumerable<object[]> GetBuffers_Interlaced() => GetBuffers(45);
    public static IEnumerable<object[]> GetBuffers_Rgb555() => GetBuffers(46);
    public static IEnumerable<object[]> GetBuffers_Clcl() => GetBuffers(47);
    public static IEnumerable<object[]> GetBuffers_Cl() => GetBuffers(48);

    [Theory]
    [MemberData(nameof(GetBuffers_Rgb565))]
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
    [MemberData(nameof(GetBuffers_Yuv422))]
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
    [MemberData(nameof(GetBuffers_Ycbcr420))]
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
    [MemberData(nameof(GetBuffers_Interlaced))]
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

    [Theory]
    [MemberData(nameof(GetBuffers_Clcl))]
    public void Fuzz_Clcl_NoCrash(byte[] buf, int w, int h)
    {
        if ((w & 1) != 0) return; // CLCL requires even width
        int allocSize = Math.Max(4096, w * h * 4);
        byte* dst = (byte*)NativeMemory.Alloc((nuint)allocSize);
        try
        {
            NativeMemory.Clear(dst, (nuint)allocSize);
            IthmbCodecPlugin.DecodeYuv422Clcl(buf, dst, w, h);
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
    [MemberData(nameof(GetBuffers_Cl))]
    public void Fuzz_Cl_NoCrash(byte[] buf, int w, int h)
    {
        int allocSize = Math.Max(4096, w * h * 4);
        byte* dst = (byte*)NativeMemory.Alloc((nuint)allocSize);
        try
        {
            NativeMemory.Clear(dst, (nuint)allocSize);
            IthmbCodecPlugin.DecodeYuv422Cl(buf, dst, w, h);
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

    // ===================== Phase 4: Robustness =====================

    // ---- Test 11: Integer overflow / extreme dimensions ----

    [Theory]
    [InlineData(100_000, 100)]    // w large but h small
    [InlineData(100, 100_000)]    // h large but w small
    [InlineData(0x7FFF, 0x7FFF)]  // near int32 overflow (w*h ≈ 2^30)
    public void Overflow_ExtremeDimensions_NoCrash(int w, int h)
    {
        // These dimensions could overflow (y * w + x) * 2 in DecodeRgb565
        // if not handled. The bounds guard should return early for undersized buffers.
        long bufLen = Math.Min((long)w * h * 2, 1024L * 1024);
        var buf = new byte[bufLen > 0 ? (int)bufLen : 1];
        new Random(42).NextBytes(buf);
        long allocSize = Math.Min((long)w * h * 4, 1024L * 1024);
        byte* dst = (byte*)NativeMemory.Alloc((nuint)Math.Max(allocSize, 1));
        try
        {
            NativeMemory.Clear(dst, (nuint)Math.Min(allocSize, 1024 * 1024));
            IthmbCodecPlugin.DecodeRgb565(buf, dst, w, h, littleEndian: true);
            // No crash = pass. Bounds guard handles undersized buffers gracefully.
        }
        finally { NativeMemory.Free(dst); }
    }

    // ---- Test 12: Determinism across all decoders ----

    [Fact]
    public void Property_Determinism_AllDecoders()
    {
        var rng = new Random(99);
        var buf = new byte[64];
        rng.NextBytes(buf);
        var buf16 = new byte[6]; // 2×2 block = 4 Y + 1 Cb + 1 Cr
        rng.NextBytes(buf16);

        // Each decoder with random input must produce identical output on repeat
        AssertDeterminism(64,  dst => IthmbCodecPlugin.DecodeRgb565(buf, (byte*)(void*)dst, 4, 4, true));
        AssertDeterminism(64,  dst => IthmbCodecPlugin.DecodeRgb555(buf, (byte*)(void*)dst, 4, 4, true));
        AssertDeterminism(64,  dst => IthmbCodecPlugin.DecodeYuv422(buf, (byte*)(void*)dst, 4, 4));
        AssertDeterminism(16,  dst => IthmbCodecPlugin.DecodeYcbcr420(buf16, (byte*)(void*)dst, 2, 2));
        AssertDeterminism(64,  dst => IthmbCodecPlugin.DecodeYuv422Interlaced(buf, (byte*)(void*)dst, 4, 4));
        AssertDeterminism(64,  dst => IthmbCodecPlugin.DecodeYuv422Clcl(buf, (byte*)(void*)dst, 4, 4));
        AssertDeterminism(64,  dst => IthmbCodecPlugin.DecodeYuv422Cl(buf, (byte*)(void*)dst, 4, 4));
    }
}
