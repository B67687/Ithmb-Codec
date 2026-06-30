using System.Collections.Concurrent;
using System.Collections.Frozen;
using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    /// <summary>
    /// Verifies that KnownProfiles can be safely read by multiple threads while
    /// a writer thread replaces the reference. The FrozenDictionary is immutable
    /// once created, so the race window is only on the reference publication.
    /// Without proper barrier semantics (Interlocked.Exchange on the writer side),
    /// a reader on a weak memory model (ARM) could observe a stale null.
    /// </summary>
    [Fact]
    [System.Diagnostics.CodeAnalysis.SuppressMessage("xUnit", "xUnit1031",
        Justification = "Cannot use async in unsafe partial class context")]
    public void KnownProfiles_ConcurrentReadDuringRebuild_NoCrash()
    {
        // Build two distinct profile dictionaries to swap during the test
        var dict1 = new Dictionary<int, IthmbCodecPlugin.IthmbVariantProfile>
        {
            [1007] = new(1007, 480, 864, IthmbCodecPlugin.IthmbEncoding.Rgb565, 480 * 864 * 2),
            [1013] = new(1013, 220, 176, IthmbCodecPlugin.IthmbEncoding.Rgb565, 220 * 176 * 2),
        }.ToFrozenDictionary();

        var dict2 = new Dictionary<int, IthmbCodecPlugin.IthmbVariantProfile>
        {
            [1015] = new(1015, 130, 88, IthmbCodecPlugin.IthmbEncoding.Rgb565, 130 * 88 * 2),
            [1016] = new(1016, 57, 57, IthmbCodecPlugin.IthmbEncoding.Rgb565, 57 * 57 * 2),
        }.ToFrozenDictionary();

        using var cts = new CancellationTokenSource();
        cts.CancelAfter(TimeSpan.FromSeconds(10));

        var exceptions = new ConcurrentQueue<Exception>();
        int readerIterations = 0;

        var original = IthmbCodecPlugin.KnownProfiles;
        try
        {
            // 8 reader threads: spin-loop reading KnownProfiles
            var readers = new Task[8];
            for (int i = 0; i < readers.Length; i++)
            {
                readers[i] = Task.Run(() =>
                {
                    while (!cts.Token.IsCancellationRequested)
                    {
                        try
                        {
                            var profiles = IthmbCodecPlugin.KnownProfiles;
                            _ = profiles?.Count;
                            Interlocked.Increment(ref readerIterations);
                        }
                        catch (Exception ex)
                        {
                            exceptions.Enqueue(ex);
                            break;
                        }
                    }
                });
            }

            var writer = Task.Run(() =>
            {
                var sw = new SpinWait();
                while (!cts.Token.IsCancellationRequested)
                {
                    IthmbCodecPlugin.KnownProfiles = dict1;
                    IthmbCodecPlugin.KnownProfiles = dict2;
                    sw.SpinOnce();
                }
            });

            Task.WaitAll([.. readers, writer]);

            Assert.True(exceptions.IsEmpty,
                $"Concurrent read/write produced exceptions: {string.Join("; ",
                    exceptions.Select(e => $"{e.GetType().Name}: {e.Message}"))}");
            Assert.True(readerIterations > 0,
                "Reader threads should have completed at least one iteration");
        }
        finally
        {
            // Restore original KnownProfiles so subsequent tests are unaffected
            IthmbCodecPlugin.KnownProfiles = original;
        }
    }
}
