using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    // ===================== Phase 1: Exhaustive + Fuzz + Properties =====================

    // ---- Test 1: Exhaustive 1×1 RGB565 (all 65,536 possible 16-bit values) ----

    [Fact]
    public void Rgb565_Exhaustive_All65536Values()
    {
        // Tests EVERY possible 16-bit RGB565 input, proving the MSB-replication
        // formula is correct for all inputs. Runs in ~50ms.
        var src = new byte[2];
        byte* dst = (byte*)NativeMemory.Alloc(4);
        try
        {
            for (int i = 0; i < 65536; i++)
            {
                src[0] = (byte)(i & 0xFF);
                src[1] = (byte)(i >> 8);

                NativeMemory.Clear(dst, 4); // zero dst before each decode
                IthmbCodecPlugin.DecodeRgb565(src, dst, 1, 1, littleEndian: true);

                // Decode the word back out
                int r5 = (i >> 11) & 0x1F;
                int g6 = (i >> 5) & 0x3F;
                int b5 = i & 0x1F;

                int expectedR = (r5 << 3) | (r5 >> 2);
                int expectedG = (g6 << 2) | (g6 >> 4);
                int expectedB = (b5 << 3) | (b5 >> 2);

                // Must match the MSB-replication formula exactly
                Assert.Equal(expectedR, dst[2]); // R
                Assert.Equal(expectedG, dst[1]); // G
                Assert.Equal(expectedB, dst[0]); // B
                Assert.Equal(255, dst[3]); // A
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    // ===================== RGB555 =====================

    // ---- P4a: Exhaustive 1×1 RGB555 (all 65,536 possible 16-bit values) ----

    [Fact]
    public void Rgb555_Exhaustive_All65536Values()
    {
        var src = new byte[2];
        byte* dst = (byte*)NativeMemory.Alloc(4);
        try
        {
            for (int i = 0; i < 65536; i++)
            {
                src[0] = (byte)(i & 0xFF);
                src[1] = (byte)(i >> 8);

                NativeMemory.Clear(dst, 4);
                IthmbCodecPlugin.DecodeRgb555(src, dst, 1, 1, littleEndian: true);

                // RGB555: xRRRRRGGGGGBBBBB
                int r5 = (i >> 10) & 0x1F;
                int g5 = (i >> 5)  & 0x1F;
                int b5 = i         & 0x1F;

                int er = (r5 << 3) | (r5 >> 2);
                int eg = (g5 << 3) | (g5 >> 2);
                int eb = (b5 << 3) | (b5 >> 2);

                Assert.Equal(er, dst[2]); // R
                Assert.Equal(eg, dst[1]); // G
                Assert.Equal(eb, dst[0]); // B
                Assert.Equal(255, dst[3]); // A
            }
        }
        finally { NativeMemory.Free(dst); }
    }

    // ---- P4b: SIMD-vs-scalar identity (w=8) ----

    [Fact]
    public void DecodeRgb555_SIMD_MatchesScalar_8Wide()
    {
        var rng = new Random(42);
        var src = new byte[8 * 2];
        byte* simdDst = (byte*)NativeMemory.Alloc(8 * 4);
        byte* scalarDst = (byte*)NativeMemory.Alloc(8 * 4);
        try
        {
            for (int i = 0; i < 65536; i += 8)
            {
                for (int j = 0; j < 8; j++)
                {
                    ushort val = (ushort)((i + j) & 0xFFFF);
                    src[j * 2] = (byte)(val & 0xFF);
                    src[j * 2 + 1] = (byte)(val >> 8);
                }

                NativeMemory.Clear(simdDst, 8 * 4);
                NativeMemory.Clear(scalarDst, 8 * 4);

                IthmbCodecPlugin.DecodeRgb555(src, simdDst, 8, 1, littleEndian: true);
                IthmbCodecPlugin.DecodeRgb555(src, scalarDst, 4, 2, littleEndian: true);

                for (int j = 0; j < 8 * 4; j++)
                    Assert.Equal(simdDst[j], scalarDst[j]);
            }
        }
        finally { NativeMemory.Free(simdDst); NativeMemory.Free(scalarDst); }
    }

    // ---- P4c: Known colors ----

    [Theory]
    [InlineData(0x0000, 0, 0, 0)]
    [InlineData(0xFFFF, 255, 255, 255)]
    [InlineData(0x7C00, 255, 0, 0)]    // R=31 (bit 15=0, bits 14-10=11111)
    [InlineData(0x03E0, 0, 255, 0)]    // G=31 (bits 9-5=11111)
    [InlineData(0x001F, 0, 0, 255)]    // B=31 (bits 4-0=11111)
    [InlineData(0x3C00, 123, 0, 0)]    // R=15 → (15<<3)|(15>>2)=120|3=123 (0x3C00 has G=0, not 0x3E00)
    public void DecodeRgb555_KnownColors(ushort rgb, int expectedR, int expectedG, int expectedB)
    {
        byte leLow = (byte)(rgb & 0xFF);
        byte leHigh = (byte)(rgb >> 8);
        byte[] src = [leLow, leHigh];
        byte* dst = (byte*)NativeMemory.Alloc(4);
        try
        {
            IthmbCodecPlugin.DecodeRgb555(src, dst, 1, 1, littleEndian: true);
            Assert.Equal(expectedB, dst[0]);
            Assert.Equal(expectedG, dst[1]);
            Assert.Equal(expectedR, dst[2]);
            Assert.Equal(255, dst[3]);
        }
        finally { NativeMemory.Free(dst); }
    }

    // ---- P4d: Big-endian ----

    [Fact]
    public void DecodeRgb555_BigEndian()
    {
        byte[] src = [0x7C, 0x00]; // BE bytes: [high=0x7C, low=0x00] → value 0x7C00 = red
        byte* dst = (byte*)NativeMemory.Alloc(4);
        try
        {
            IthmbCodecPlugin.DecodeRgb555(src, dst, 1, 1, littleEndian: false);
            Assert.Equal(0, dst[0]);   // B
            Assert.Equal(0, dst[1]);   // G
            Assert.Equal(255, dst[2]); // R
        }
        finally { NativeMemory.Free(dst); }
    }

    // ---- P4e: Fuzz (50 random buffers) ----

    [Theory]
    [MemberData(nameof(GetRandomValidBuffers))]
    public void Fuzz_Rgb555_NoCrash(byte[] buf, int w, int h)
    {
        int allocSize = Math.Max(4096, w * h * 4);
        byte* dst = (byte*)NativeMemory.Alloc((nuint)allocSize);
        try
        {
            NativeMemory.Clear(dst, (nuint)allocSize);
            IthmbCodecPlugin.DecodeRgb555(buf, dst, w, h, littleEndian: true);
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

    // ---- P4f: Determinism ----

    [Fact]
    public void Property_Determinism_Rgb555()
    {
        byte[] src = [0x00, 0x7C]; // LE 0x7C00 = red
        byte* dst1 = (byte*)NativeMemory.Alloc(4);
        byte* dst2 = (byte*)NativeMemory.Alloc(4);
        try
        {
            IthmbCodecPlugin.DecodeRgb555(src, dst1, 1, 1, littleEndian: true);
            IthmbCodecPlugin.DecodeRgb555(src, dst2, 1, 1, littleEndian: true);
            for (int i = 0; i < 4; i++)
                Assert.Equal(dst1[i], dst2[i]);
        }
        finally { NativeMemory.Free(dst1); NativeMemory.Free(dst2); }
    }

    // ---- P4g: Roundtrip ----

    [Fact]
    public void Rgb555_Roundtrip_AllCornerColors()
    {
        int[] vals = [0, 64, 128, 192, 255];
        foreach (int r in vals)
        {
            foreach (int g in vals)
            {
                foreach (int b in vals)
                {
                    // Pack reference: xRRRRRGGGGGBBBBB
                    int r5 = r >> 3;
                    int g5 = g >> 3;
                    int b5 = b >> 3;
                    ushort packed = (ushort)((r5 << 10) | (g5 << 5) | b5);

                    byte leLow = (byte)(packed & 0xFF);
                    byte leHigh = (byte)(packed >> 8);
                    byte[] src = [leLow, leHigh];
                    byte* dst = (byte*)NativeMemory.Alloc(4);
                    try
                    {
                        IthmbCodecPlugin.DecodeRgb555(src, dst, 1, 1, littleEndian: true);
                        int dr = Math.Abs(dst[2] - r);
                        int dg = Math.Abs(dst[1] - g);
                        int db = Math.Abs(dst[0] - b);
                        Assert.True(dr <= 8, $"R roundtrip error: {r}→{dst[2]} (Δ{dr})");
                        Assert.True(dg <= 8, $"G roundtrip error: {g}→{dst[1]} (Δ{dg})");
                        Assert.True(db <= 8, $"B roundtrip error: {b}→{dst[0]} (Δ{db})");
                    }
                    finally { NativeMemory.Free(dst); }
                }
            }
        }
    }

    // ---- P4h: New profiles exist ----

    [Fact]
    public void NewProfiles_Exist()
    {
        // Verify all newly added profiles are in the KnownProfiles dictionary
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(1005));
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(1032));
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(1092));
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(1093));
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(3004));
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(3009));
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(3011));
        // Cover art profiles
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(1016));
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(1017));
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(1028));
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(1029));
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(1055));
        Assert.True(IthmbCodecPlugin.KnownProfiles.ContainsKey(1060));
    }
}
