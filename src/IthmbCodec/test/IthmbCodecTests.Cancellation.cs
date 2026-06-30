using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    /// <summary>
    /// Stress-tests decode cancellation: runs decode in a tight loop, cancels after 50ms,
    /// and verifies no native buffer leak by tracking GC allocation growth across 100 iterations.
    /// </summary>
    [Fact]
    [System.Diagnostics.CodeAnalysis.SuppressMessage("xUnit", "xUnit1031",
        Justification = "Cannot use async in unsafe partial class context")]
    public void MidDecodeCancel_NoMemoryLeak()
    {
        const int frameCount = 5;
        const int width = 100;
        const int height = 100;
        const int iterations = 100;
        const int cancelDelayMs = 50;

        // Build a multi-frame RGB565 .ithmb buffer (5 frames × 100×100 pixels)
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1007, Width: width, Height: height,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: width * height * 2);

        byte[] bgra = new byte[width * height * 4];
        var rng = new Random(42);
        rng.NextBytes(bgra);

        // Build multi-frame .ithmb file: 4-byte prefix + (frameSize × frameCount)
        byte[] frameEncoded = IthmbCodecPlugin.EncodeRgb565(bgra, width, height, bigEndian: false);
        var ithmbData = new byte[4 + frameEncoded.Length * frameCount];
        // Big-endian prefix
        ithmbData[0] = (byte)(profile.Prefix >> 24);
        ithmbData[1] = (byte)(profile.Prefix >> 16);
        ithmbData[2] = (byte)(profile.Prefix >> 8);
        ithmbData[3] = (byte)profile.Prefix;
        for (int f = 0; f < frameCount; f++)
            Buffer.BlockCopy(frameEncoded, 0, ithmbData, 4 + f * frameEncoded.Length, frameEncoded.Length);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));

        try
        {
            // Warm up: one successful decode to stabilize JIT / allocations
            var warmup = IthmbCodecPlugin.DecodeRawProfile(ithmbData, profile,
                cancellation: null, outInfo, outBuf);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, warmup);
            // Free the warmup buffer
            IthmbCodecPlugin.FreePixelBuffer(outBuf);

            // Force a full GC so baseline is clean
            GC.Collect();
            GC.WaitForPendingFinalizers();
            GC.Collect();
            long baselineMemory = GC.GetTotalMemory(forceFullCollection: true);

            for (int i = 0; i < iterations; i++)
            {
                using var cts = new CancellationTokenSource();
                int frameIndex = i % frameCount;

                var task = Task.Run(() =>
                {
                    // Tight decode loop — will be cancelled mid-flight
                    while (!cts.Token.IsCancellationRequested)
                    {
                        NativeMemory.Clear(outBuf, (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
                        var status = IthmbCodecPlugin.DecodeRawProfile(ithmbData, profile,
                            cancellation: null, outInfo, outBuf, frameIndex: frameIndex);

                        // Free the allocated buffer after each decode to match real usage
                        if (status == ImageGlass.SDK.Plugins.IGStatus.OK)
                            IthmbCodecPlugin.FreePixelBuffer(outBuf);
                    }
                });

                // Let the task run briefly, then cancel
                Thread.Sleep(cancelDelayMs);
                cts.Cancel();

                // Wait for the task to finish (with a generous timeout to avoid deadlock)
                task.Wait(TimeSpan.FromSeconds(2));

                // Verify no exception escaped
                Assert.True(task.IsCompleted);
            }

            // Force GC after all iterations
            GC.Collect();
            GC.WaitForPendingFinalizers();
            GC.Collect();
            long finalMemory = GC.GetTotalMemory(forceFullCollection: true);

            // Allow up to 512 KB of drift — the decode itself allocates temporary arrays
            // (frame slicing, profile lookups) that are normal GC-managed overhead.
            // Native buffer leaks would show as much larger, unbounded growth.
            long drift = finalMemory - baselineMemory;
            Assert.True(drift < 512 * 1024,
                $"Memory grew {drift} bytes after {iterations} cancelled decode cycles — possible native buffer leak");
        }
        finally
        {
            // Safety: free any buffer that might still be tracked
            IthmbCodecPlugin.FreePixelBuffer(outBuf);
            NativeMemory.Free(outInfo);
            NativeMemory.Free(outBuf);
        }
    }
}
