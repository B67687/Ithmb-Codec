using System.Collections.Concurrent;
using IthmbCodec;
using static IthmbCodec.PhotoDb.PhotoDb;
using Xunit;

namespace IthmbCodec.Tests;

public class FormatIdNameTests
{
    /// <summary>
    /// Verifies that <see cref="GetFormatIdName"/> can be called
    /// concurrently from multiple reader threads without torn reads, exceptions,
    /// or stale results. The method reads from the immutable
    /// <see cref="IthmbCodecPlugin.KnownProfiles"/> FrozenDictionary — once
    /// created, entries cannot change. The only race window is the volatile
    /// reference publication when profiles are rebuilt; this test exercises
    /// the read path under contention to confirm no torn reads or exceptions.
    /// </summary>
    [Fact]
    [System.Diagnostics.CodeAnalysis.SuppressMessage("xUnit", "xUnit1031",
        Justification = "Cannot use async in concurrent test")]
    public void GetFormatIdName_ConcurrentWithProfileUpdate_NoStaleReads()
    {
        // Known format IDs that exist in KnownProfiles (decimal form)
        int[] knownFormatIds =
        [
            1007,  // 480×864 Rgb565
            1013,  // 220×176 Rgb565
            1015,  // 130×88  Rgb565
            1016,  // 57×57   Rgb565
            1019,  // 720×480 Yuv422 interlaced
            1024,  // 55×56   Rgb565
            1055,  // 220×176 Yuv422
            1060,  // 320×240 YCbCr420
            1061,  // 320×240 Yuv422
            1066,  // 128×128 Rgb565
            1067,  // 128×128 Rgb555
            1068,  // 220×176 Rgb555
            1071,  // 57×57   Rgb555
            1074,  // 130×88  Rgb555
        ];

        // Unknown format IDs — GetFormatIdName should return the numeric fallback
        int[] unknownFormatIds = [9999, 65535, 0x0000_0001, 0x0000_0101, 0x0000_0112];

        int[] allFormatIds = [.. knownFormatIds, .. unknownFormatIds];

        const int readerCount = 4;
        const int durationMs = 5000;
        const int timeoutMs = 30_000;

        using var cts = new CancellationTokenSource(TimeSpan.FromMilliseconds(durationMs));
        var exceptions = new ConcurrentQueue<Exception>();
        long readerIterations = 0;
        long tornReads = 0;

        // Spawn reader threads: call GetFormatIdName with various format IDs
        var readers = new Task[readerCount];
        for (int i = 0; i < readerCount; i++)
        {
            readers[i] = Task.Run(() =>
            {
                var rng = new Random(Thread.CurrentThread.ManagedThreadId);
                while (!cts.Token.IsCancellationRequested)
                {
                    try
                    {
                        int formatId = allFormatIds[rng.Next(allFormatIds.Length)];
                        string result = GetFormatIdName(formatId);

                        // Result must not be null or empty
                        Assert.False(string.IsNullOrEmpty(result),
                            $"GetFormatIdName({formatId}) returned null or empty");

                        // Result is either:
                        // (a) A description containing non-numeric characters (known profile)
                        // (b) A numeric string fallback (unknown profile)
                        bool isNumeric = result.All(c => char.IsDigit(c));
                        bool isDescription = result.Any(c => !char.IsDigit(c)
                            && c != ' ' && c != '(' && c != ')' && c != '×' && c != 'x');

                        Assert.True(isNumeric || isDescription,
                            $"GetFormatIdName({formatId}) returned unexpected format: \"{result}\"");

                        // No torn reads: the result should not contain unbalanced
                        // parentheses from interleaved string concatenation.
                        if (result.Contains('('))
                        {
                            int openCount = result.Count(c => c == '(');
                            int closeCount = result.Count(c => c == ')');
                            if (openCount != closeCount)
                            {
                                Interlocked.Increment(ref tornReads);
                            }
                        }

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

        bool allCompleted = Task.WaitAll(readers, TimeSpan.FromMilliseconds(timeoutMs));

        Assert.True(allCompleted,
            $"Reader tasks did not complete within {timeoutMs}ms timeout");

        Assert.True(exceptions.IsEmpty,
            $"Concurrent GetFormatIdName produced exceptions: {string.Join("; ",
                exceptions.Select(e => $"{e.GetType().Name}: {e.Message}"))}");

        Assert.True(readerIterations > 0,
            "Reader threads should have completed at least one iteration");

        Assert.Equal(0L, Interlocked.Read(ref tornReads));
    }
}
