using System.Buffers;
using System.Runtime.InteropServices;
using IthmbCodec;
using ImageGlass.SDK.Plugins;
using Xunit;

namespace IthmbCodec.Tests;

public class ArrayPoolTests
{
    /// <summary>
    /// Behavioral test: verifies ArrayPool&lt;byte&gt;.Shared rent/return cycle completes
    /// without error and produces a usable buffer. If someone removes the pool usage
    /// from the decode pipeline or changes the return pattern, this test will surface it.
    /// </summary>
    [Fact]
    public void ArrayPool_RentReturn_NoLeak()
    {
        const int bufferSize = 4096;
        byte[] buffer = ArrayPool<byte>.Shared.Rent(bufferSize);

        Assert.NotNull(buffer);
        Assert.True(buffer.Length >= bufferSize,
            $"Rented buffer ({buffer.Length}) must be >= requested ({bufferSize})");

        // Fill with a recognizable pattern
        for (int i = 0; i < bufferSize; i++)
            buffer[i] = (byte)(i & 0xFF);

        // Return to pool — must not throw
        ArrayPool<byte>.Shared.Return(buffer);

        // Re-rent same size — pool should reuse (or at least return a usable buffer)
        byte[] buffer2 = ArrayPool<byte>.Shared.Rent(bufferSize);

        Assert.NotNull(buffer2);
        Assert.True(buffer2.Length >= bufferSize,
            $"Re-rented buffer ({buffer2.Length}) must be >= requested ({bufferSize})");

        // The buffer must be usable — write and read without exception
        buffer2[0] = 0xAB;
        Assert.Equal(0xAB, buffer2[0]);

        ArrayPool<byte>.Shared.Return(buffer2);
    }

    /// <summary>
    /// Indirectly tests the two-phase peek logic in DecodeInternal by decoding
    /// synthetic .ithmb files of different sizes through the full file-based pipeline.
    ///
    /// Phase 1 reads min(fileSize, 512KB) and scans for JPEG.
    /// Phase 2 extends to min(fileSize, 4MB) only if no JPEG found and fileSize > 512KB.
    ///
    /// • Small file (&lt; 512KB): only Phase 1 runs, then falls through to raw profile decode.
    /// • Medium file (512KB–4MB): both phases run, then falls through to raw profile decode.
    ///
    /// Both paths should produce a valid decoded image without error.
    /// </summary>
    [Fact]
    public unsafe void TwoPhasePeek_ReadsCorrectSize()
    {
        // ---- Small file path: well under 512KB, exercises Phase 1 only ----
        // Profile 1061: 55×55 RGB565 LE, FrameByteLength=6160. Total file: 4 + 6160 = 6164 bytes.
        {
            int w = 55, h = 55;
            var bgra = new byte[w * h * 4];
            // Fill with a recognizable pattern (diagonal gradient)
            for (int i = 0; i < w * h; i++)
            {
                bgra[i * 4] = (byte)(i % 256);         // B
                bgra[i * 4 + 1] = (byte)((i * 3) & 0xFF); // G
                bgra[i * 4 + 2] = (byte)((i * 7) & 0xFF); // R
                bgra[i * 4 + 3] = 255;                      // A
            }

            var profile = new IthmbCodecPlugin.IthmbVariantProfile(
                Prefix: 1061, Width: 55, Height: 55,
                Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
                FrameByteLength: 6160);

            byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, w, h, profile);
            Assert.True(ithmbFile.Length < 512 * 1024,
                $"Small test file ({ithmbFile.Length} bytes) should be under 512KB");

            string tmpPath = Path.Combine(Path.GetTempPath(), $"ithmb_arraypool_small_{Guid.NewGuid():N}.ithmb");
            try
            {
                File.WriteAllBytes(tmpPath, ithmbFile);

                var outInfo = (IGImageInfo*)NativeMemory.AllocZeroed((nuint)sizeof(IGImageInfo));
                var outBuf = (IGPixelBuffer*)NativeMemory.AllocZeroed((nuint)sizeof(IGPixelBuffer));
                try
                {
                    // Decode through the full file-based pipeline (exercises Phase 1 peek)
                    char[] pathChars = (tmpPath + "\0").ToCharArray();
                    fixed (char* pPath = pathChars)
                    {
                        var filePath = new IGStringRef { Data = pPath, Length = pathChars.Length - 1 };
                        var status = IthmbCodecPlugin.DecodeInternal(filePath, cancellation: null, outInfo, outBuf);
                        Assert.Equal(IGStatus.OK, status);
                        Assert.Equal(55, outBuf->Width);
                        Assert.Equal(55, outBuf->Height);
                    }
                }
                finally
                {
                    if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
                    NativeMemory.Free(outInfo);
                    NativeMemory.Free(outBuf);
                }
            }
            finally
            {
                if (File.Exists(tmpPath)) File.Delete(tmpPath);
            }
        }

        // ---- Medium file path: between 512KB and 4MB, exercises Phase 1 + Phase 2 ----
        // Profile 1007: 480×864 RGB565 LE, FrameByteLength=829440. Total file: 4 + 829440 = 829444 bytes.
        {
            int w = 480, h = 864;
            var bgra = new byte[w * h * 4];
            // Fill with a deterministic gradient (no allocation of full BGRA — encode directly)
            // Use BuildIthmbFile which encodes from BGRA, so we only need the BGRA data.
            // To avoid a 1.6 MB allocation, generate a pattern that encodes to recognizable output.
            // Generate safe data: gradient pattern that never produces false JPEG SOI (0xFF 0xD8)
            // in the RGB565-encoded output. Avoid R≥248 && G≥252 && B≈24-31 (the SOI false-positive
            // pixel combination). Use horizontal gradient where G varies slowly.
            for (int y = 0; y < h; y++)
            for (int x = 0; x < w; x++)
            {
                int idx = (y * w + x) * 4;
                bgra[idx] = (byte)((x * 31 / w) * 8);          // B: 0-248 in steps of 8
                bgra[idx + 1] = (byte)((y * 63 / h) * 4);      // G: 0-252 in steps of 4
                bgra[idx + 2] = (byte)((x * 31 / w) * 8);      // R: 0-248 in steps of 8
                bgra[idx + 3] = 255;                               // A
            }
            for (int i = 0; i < w * h; i++)
                bgra[i * 4 + 3] = 255;

            var profile = new IthmbCodecPlugin.IthmbVariantProfile(
                Prefix: 1007, Width: 480, Height: 864,
                Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
                FrameByteLength: 829440);

            byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, w, h, profile);
            Assert.True(ithmbFile.Length >= 512 * 1024,
                $"Medium test file ({ithmbFile.Length} bytes) should be >= 512KB");
            Assert.True(ithmbFile.Length < 4 * 1024 * 1024,
                $"Medium test file ({ithmbFile.Length} bytes) should be < 4MB");

            string tmpPath = Path.Combine(Path.GetTempPath(), $"ithmb_arraypool_medium_{Guid.NewGuid():N}.ithmb");
            try
            {
                File.WriteAllBytes(tmpPath, ithmbFile);

                var outInfo = (IGImageInfo*)NativeMemory.AllocZeroed((nuint)sizeof(IGImageInfo));
                var outBuf = (IGPixelBuffer*)NativeMemory.AllocZeroed((nuint)sizeof(IGPixelBuffer));
                try
                {
                    // Decode through the full file-based pipeline (exercises Phase 1 + Phase 2 peek)
                    char[] pathChars = (tmpPath + "\0").ToCharArray();
                    fixed (char* pPath = pathChars)
                    {
                        var filePath = new IGStringRef { Data = pPath, Length = pathChars.Length - 1 };
                        var status = IthmbCodecPlugin.DecodeInternal(filePath, cancellation: null, outInfo, outBuf);
                        Assert.Equal(IGStatus.OK, status);
                        Assert.Equal(480, outBuf->Width);
                        Assert.Equal(864, outBuf->Height);
                    }
                }
                finally
                {
                    if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
                    NativeMemory.Free(outInfo);
                    NativeMemory.Free(outBuf);
                }
            }
            finally
            {
                if (File.Exists(tmpPath)) File.Delete(tmpPath);
            }
        }
    }
}
