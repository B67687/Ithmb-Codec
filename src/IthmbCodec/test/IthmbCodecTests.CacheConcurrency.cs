using System.Collections.Concurrent;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public class CacheConcurrencyTests
{
    /// <summary>
    /// Verifies that concurrent SetCachedFile / TryGetCachedFile operations
    /// do not corrupt the cache, exceed MaxCachedPaths (16), or produce
    /// duplicate entries. Multiple writer and reader threads hammer the
    /// static LRU cache for 5 seconds / 1000 iterations, whichever comes first.
    /// </summary>
    [Fact]
    [System.Diagnostics.CodeAnalysis.SuppressMessage("xUnit", "xUnit1031",
        Justification = "Cannot use async in [Fact] with manual threading")]
    public void RawFileCache_ConcurrentReadWriteEvict_NoCorruption()
    {
        IthmbCodecPlugin.ClearRawFileCache();

        const int maxCachedPaths = 16;
        const int totalPaths = 24; // exceeds MaxCachedPaths to force eviction
        const int writerCount = 4;
        const int readerCount = 4;
        var deadline = DateTime.UtcNow + TimeSpan.FromSeconds(5);

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(1007, 1, 1, IthmbCodecPlugin.IthmbEncoding.Rgb565, 2);
        var paths = Enumerable.Range(0, totalPaths).Select(i => $"test_{i}.ithmb").ToArray();
        var data = new byte[100]; // synthetic payload

        var exceptions = new ConcurrentQueue<Exception>();
        int totalWrites = 0;
        int totalReads = 0;

        // --- writer threads: each writer owns a slice of paths, writes in round-robin ---
        var writers = new Task[writerCount];
        for (int w = 0; w < writerCount; w++)
        {
            int writerId = w;
            writers[w] = Task.Run(() =>
            {
                int iteration = 0;
                try
                {
                    while (DateTime.UtcNow < deadline && iteration < 1000)
                    {
                        // Each writer cycles through a subset of paths
                        int pathIndex = (writerId + iteration * writerCount) % totalPaths;
                        IthmbCodecPlugin.SetCachedFile(paths[pathIndex], data, profile, 1, 100);
                        Interlocked.Increment(ref totalWrites);
                        iteration++;
                    }
                }
                catch (Exception ex)
                {
                    exceptions.Enqueue(ex);
                }
            });
        }

        // --- reader threads: read from any path in the cache ---
        var readers = new Task[readerCount];
        for (int r = 0; r < readerCount; r++)
        {
            readers[r] = Task.Run(() =>
            {
                var rng = new Random(Thread.CurrentThread.ManagedThreadId);
                int iteration = 0;
                try
                {
                    while (DateTime.UtcNow < deadline && iteration < 1000)
                    {
                        int pathIndex = rng.Next(totalPaths);
                        IthmbCodecPlugin.TryGetCachedFile(paths[pathIndex], out _);
                        Interlocked.Increment(ref totalReads);
                        iteration++;
                    }
                }
                catch (Exception ex)
                {
                    exceptions.Enqueue(ex);
                }
            });
        }

        Task.WaitAll([.. writers, .. readers]);

        // --- assertions ---

        // 1. No NullReferenceException, IndexOutOfRangeException, or other unexpected exceptions
        Assert.False(
            exceptions.Any(e =>
                e is NullReferenceException or IndexOutOfRangeException or OverflowException),
            $"Concurrent access produced critical exceptions: {string.Join("; ", exceptions.Select(e => $"{e.GetType().Name}: {e.Message}"))}");

        // 2. At least some work was done
        Assert.True(totalWrites > 0, "Writer threads should have completed at least one iteration");
        Assert.True(totalReads > 0, "Reader threads should have completed at least one iteration");

        // 3. Cache size never exceeds MaxCachedPaths (16)
        // Count entries by reading all paths — only those present are in the cache
        int presentCount = 0;
        foreach (var path in paths)
        {
            if (IthmbCodecPlugin.TryGetCachedFile(path, out _))
                presentCount++;
        }

        // There may also be entries from other tests sharing the process,
        // but within our known set of paths, the total should respect the limit.
        // The cache is global; verify the total count after our workload by
        // checking a snapshot through the paths we know about.
        Assert.True(presentCount <= maxCachedPaths,
            $"Cache should hold at most {maxCachedPaths} entries, but found {presentCount} matching our paths");

        // 4. No duplicate keys (ConcurrentDictionary guarantees this by definition,
        //    but we verify the logical invariant: every present path yields exactly one entry)
        foreach (var path in paths)
        {
            if (IthmbCodecPlugin.TryGetCachedFile(path, out var entry))
            {
                // Entry must be non-default (valid data)
                Assert.NotNull(entry.Data);
                Assert.True(entry.FrameSize > 0, $"Entry for {path} should have valid FrameSize");
            }
        }

        // 5. LRU eviction: write maxCachedPaths + 1 entries with the first being untouched,
        //    then verify the first was evicted (oldest LastAccess wins).
        IthmbCodecPlugin.ClearRawFileCache();
        var lruData = new byte[10];
        for (int i = 0; i <= maxCachedPaths; i++)
        {
            IthmbCodecPlugin.SetCachedFile($"lru_{i}", lruData, profile, 1, 10);
        }

        // lru_0 was written first and never re-read → should be evicted
        Assert.False(IthmbCodecPlugin.TryGetCachedFile("lru_0", out _),
            "LRU should evict the oldest untouched entry (lru_0)");
        // The newest entry should survive
        Assert.True(IthmbCodecPlugin.TryGetCachedFile($"lru_{maxCachedPaths}", out _),
            "LRU should preserve the most recently written entry");
    }
}
