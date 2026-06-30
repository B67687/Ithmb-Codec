using System.Collections.Concurrent;
using System.Collections.Frozen;
using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    // Test-only target for concurrent reference swap tests.
    // Do NOT use IthmbCodecPlugin.KnownProfiles directly — that static is
    // shared across all tests; corrupting it would break parallel test classes.
    private static volatile FrozenDictionary<int, IthmbCodecPlugin.IthmbVariantProfile> _testSwapTarget =
        IthmbCodecPlugin.KnownProfiles;

    [Fact]
    [System.Diagnostics.CodeAnalysis.SuppressMessage("xUnit", "xUnit1031",
        Justification = "Cannot use async in unsafe partial class context")]
    public void KnownProfiles_ConcurrentReadDuringRebuild_NoCrash()
    {
        // Build two distinct FrozenDictionary instances with KNOWN format IDs
        // that match the real KnownProfiles. Both dicts include 1017 and 1031
        // so that PhotoDbRoundtripTests (which may run in parallel) always
        // finds valid profiles even if it observes _testSwapTarget.
        var dict1 = new Dictionary<int, IthmbCodecPlugin.IthmbVariantProfile>
        {
            [1007] = new(1007, 480, 864, IthmbCodecPlugin.IthmbEncoding.Rgb565, 480 * 864 * 2),
            [1013] = new(1013, 220, 176, IthmbCodecPlugin.IthmbEncoding.Rgb565, 220 * 176 * 2),
            [1017] = new(1017, 56, 56, IthmbCodecPlugin.IthmbEncoding.Rgb565, 56 * 56 * 2),
            [1031] = new(1031, 42, 42, IthmbCodecPlugin.IthmbEncoding.Rgb565, 42 * 42 * 2),
        }.ToFrozenDictionary();

        var dict2 = new Dictionary<int, IthmbCodecPlugin.IthmbVariantProfile>
        {
            [1015] = new(1015, 130, 88, IthmbCodecPlugin.IthmbEncoding.Rgb565, 130 * 88 * 2),
            [1016] = new(1016, 57, 57, IthmbCodecPlugin.IthmbEncoding.Rgb565, 57 * 57 * 2),
            [1017] = new(1017, 56, 56, IthmbCodecPlugin.IthmbEncoding.Rgb565, 56 * 56 * 2),
            [1031] = new(1031, 42, 42, IthmbCodecPlugin.IthmbEncoding.Rgb565, 42 * 42 * 2),
        }.ToFrozenDictionary();

        const int readerCount = 4;
        const int writerCount = 1;
        const int totalParticipants = readerCount + writerCount;
        const int perParticipantIterations = 1000;

        using var barrier = new Barrier(totalParticipants);
        var exceptions = new ConcurrentQueue<Exception>();
        int readerIterations = 0;

        try
        {
            var tasks = new Task[totalParticipants];

            // Start readers
            for (int i = 0; i < readerCount; i++)
            {
                tasks[i] = Task.Run(() =>
                {
                    // Signal arrival at the barrier. All 5 participants (4 readers + 1 writer)
                    // must reach this point before any proceeds — guaranteeing that every
                    // participant is actively running before the first iteration.
                    barrier.SignalAndWait();

                    for (int j = 0; j < perParticipantIterations; j++)
                    {
                        try
                        {
                            var profiles = _testSwapTarget;
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

            // Start writer
            tasks[readerCount] = Task.Run(() =>
            {
                barrier.SignalAndWait();

                for (int j = 0; j < perParticipantIterations; j++)
                {
                    _testSwapTarget = dict1;
                    _testSwapTarget = dict2;
                    Thread.Sleep(1);
                }
            });

            // Wait for all tasks with a generous timeout (safety net only)
            bool allCompleted = Task.WaitAll(tasks, TimeSpan.FromSeconds(30));

            Assert.True(allCompleted,
                $"Tasks did not complete within 30s timeout (readerIterations={readerIterations})");
            Assert.True(exceptions.IsEmpty,
                $"Concurrent read/write produced exceptions: {string.Join("; ",
                    exceptions.Select(e => $"{e.GetType().Name}: {e.Message}"))}");
            Assert.True(readerIterations > 0,
                "Reader threads should have completed at least one iteration");
        }
        finally
        {
            _testSwapTarget = IthmbCodecPlugin.KnownProfiles;
        }
    }
}
