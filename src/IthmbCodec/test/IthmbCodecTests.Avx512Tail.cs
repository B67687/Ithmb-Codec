// SPDX-License-Identifier: MIT
// AVX-512 tail path correctness: compare SSE2 vs AVX-512 output at width boundaries

using System.Runtime.Intrinsics.X86;
using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe class Avx512TailTests
{
    /// <summary>
    /// Decodes RGB565 source via the public DecodeRgb565 API with the given row geometry,
    /// which forces a specific SIMD dispatch path based on width.
    /// </summary>
    private static void DecodeRgb565Helper(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        IthmbCodecPlugin.DecodeRgb565(src, dst, w, h, littleEndian: true);
    }

    /// <summary>
    /// Computes expected BGRA output for a single RGB565 pixel using the reference formula.
    /// </summary>
    private static void DecodeRgb565Scalar(ushort rgb, out byte b, out byte g, out byte r)
    {
        int r5 = (rgb >> 11) & 0x1F;
        int g6 = (rgb >> 5) & 0x3F;
        int b5 = rgb & 0x1F;
        b = (byte)((b5 << 3) | (b5 >> 2));
        g = (byte)((g6 << 2) | (g6 >> 4));
        r = (byte)((r5 << 3) | (r5 >> 2));
    }

    /// <summary>
    /// For each boundary width, decode the same random RGB565 data via two different
    /// dispatch paths and verify they produce identical pixel output.
    ///
    /// The trick: the same N pixels of source data can be decoded with different (w, h)
    /// geometries to force different SIMD code paths:
    ///   - w=width, h=1        → auto-dispatches (SSE2 for w&lt;32, AVX-512 for w&gt;=32)
    ///   - w=8, h=totalPixels/8 → forces SSE2 path for each row (w=8 triggers SSE2)
    ///   - Scalar reference      → pixel-by-pixel reference formula
    ///
    /// Widths not divisible by the SIMD stride exercise the tail path.
    /// </summary>
    [Fact]
    public void Avx512Tail_CompareSse2VsAvx512_WidthBoundaries()
    {
        if (!Avx512BW.IsSupported)
            return; // Skip on non-AVX-512 hardware

        int[] widths = [1, 31, 32, 33, 47, 63, 64, 65];
        var rng = new Random(42);

        foreach (int width in widths)
        {
            // Create source buffer: width pixels × 2 bytes each
            var src = new byte[width * 2];
            rng.NextBytes(src);

            // --- Path 1: Auto-dispatch (SSE2 for w<32, AVX-512 for w>=32) ---
            byte* dstAuto = (byte*)NativeMemory.Alloc((nuint)(width * 4));
            // --- Path 2: Scalar reference (w=1 forces pure scalar) ---
            byte* dstScalar = (byte*)NativeMemory.Alloc((nuint)(width * 4));
            try
            {
                NativeMemory.Clear(dstAuto, (nuint)(width * 4));
                NativeMemory.Clear(dstScalar, (nuint)(width * 4));

                // Auto-dispatch: uses AVX-512 when w>=32 on AVX-512 hardware
                DecodeRgb565Helper(src, dstAuto, width, 1);

                // Scalar reference: w=1 → scalar path for every pixel
                DecodeRgb565Helper(src, dstScalar, 1, width);

                // Verify auto-dispatch matches scalar reference
                for (int i = 0; i < width; i++)
                {
                    int off = i * 4;
                    Assert.Equal(dstScalar[off], dstAuto[off]);       // B
                    Assert.Equal(dstScalar[off + 1], dstAuto[off + 1]); // G
                    Assert.Equal(dstScalar[off + 2], dstAuto[off + 2]); // R
                    Assert.Equal(255, dstAuto[off + 3]);                // A
                }

                // --- Path 3: Force SSE2 via row splitting ---
                // Decode the same data as rows of 8 pixels (SSE2 territory)
                // This only works when total pixels is divisible by 8
                if (width >= 8 && width % 8 == 0)
                {
                    byte* dstSse2 = (byte*)NativeMemory.Alloc((nuint)(width * 4));
                    try
                    {
                        NativeMemory.Clear(dstSse2, (nuint)(width * 4));
                        // w=8, h=width/8 → each row processed by SSE2 (8 pixels)
                        DecodeRgb565Helper(src, dstSse2, 8, width / 8);

                        // SSE2 result must match scalar reference
                        for (int i = 0; i < width; i++)
                        {
                            int off = i * 4;
                            Assert.Equal(dstScalar[off], dstSse2[off]);       // B
                            Assert.Equal(dstScalar[off + 1], dstSse2[off + 1]); // G
                            Assert.Equal(dstScalar[off + 2], dstSse2[off + 2]); // R
                            Assert.Equal(255, dstSse2[off + 3]);                // A
                        }
                    }
                    finally { NativeMemory.Free(dstSse2); }
                }

                // --- Verify against RGB565 formula (independent reference) ---
                for (int i = 0; i < width; i++)
                {
                    ushort pixel = (ushort)(src[i * 2] | (src[i * 2 + 1] << 8));
                    DecodeRgb565Scalar(pixel, out byte expB, out byte expG, out byte expR);
                    int off = i * 4;
                    Assert.Equal(expB, dstAuto[off]);       // B
                    Assert.Equal(expG, dstAuto[off + 1]);   // G
                    Assert.Equal(expR, dstAuto[off + 2]);   // R
                    Assert.Equal(255, dstAuto[off + 3]);     // A
                }
            }
            finally
            {
                NativeMemory.Free(dstAuto);
                NativeMemory.Free(dstScalar);
            }
        }
    }
}
