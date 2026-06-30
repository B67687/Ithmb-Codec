// Decode infrastructure for .ithmb files: raw file cache (bounded LRU) and decode
// performance metrics. Separated from the decode pipeline for independent compilation
// and file-size discipline.

using System.Buffers;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.Threading;
using ImageGlass.SDK.Plugins;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // Cache for multi-frame raw .ithmb files. Populated by the first DecodeInternal
    // call for a raw file, reused across subsequent frameIndex values without re-reading.
    // Read-once, decode-many: the ithmb file is read in full once and cached here.
    //
    // Eviction policy: LRU bounded cache — when the number of cached paths exceeds
    // MaxCachedPaths (16), the entry with the oldest LastAccess timestamp is evicted.
    // TryGetCachedFile updates LastAccess on each hit, so recently accessed entries are
    // retained. This bounds memory growth from every unique file path ever decoded
    // accumulating a full byte[] (up to 32 MB).
    private const int MaxCachedPaths = 16;
    private const int MaxCarvingFileSize = 8 * 1024 * 1024; // 8 MB: skip JPEG carving on oversized unknown-prefix files
    private static readonly ConcurrentDictionary<string, RawFileCacheEntry> _rawFileCache = new();

    // Decode performance metrics (Interlocked for thread safety across concurrent decode threads)
    private static long _decodeCount;
    private static long _decodeSuccessCount;
    private static long _decodeTotalTicks;

    internal static void SetCachedFile(string path, byte[] data, IthmbVariantProfile profile, int frameCount, int frameSize)
    {
        // LRU eviction: when MaxCachedPaths is exceeded, remove the oldest entry.
        // ConcurrentDictionary iteration is safe (returns a snapshot), and TryRemove
        // handles the race if another thread removes the same key concurrently.
        if (_rawFileCache.Count >= MaxCachedPaths)
        {
            long oldestTs = long.MaxValue;
            string? oldestKey = null;
            foreach (var kvp in _rawFileCache)
            {
                if (kvp.Value.LastAccess < oldestTs)
                {
                    oldestTs = kvp.Value.LastAccess;
                    oldestKey = kvp.Key;
                }
            }
            if (oldestKey != null)
                _rawFileCache.TryRemove(oldestKey, out _);
        }
        _rawFileCache[path] = new RawFileCacheEntry(data, profile, frameCount, frameSize)
        {
            LastAccess = Stopwatch.GetTimestamp()
        };
    }

    /// <summary>Gets a cached file entry and updates its LastAccess timestamp.</summary>
    internal static bool TryGetCachedFile(string path, out RawFileCacheEntry entry)
    {
        if (_rawFileCache.TryGetValue(path, out entry))
        {
            entry.LastAccess = Stopwatch.GetTimestamp();
            _rawFileCache[path] = entry;
            return true;
        }
        return false;
    }

    // Test support — resets the raw file cache to empty.
    internal static void ClearRawFileCache() => _rawFileCache.Clear();

    /// <summary>Returns cumulative decode metrics for observability.</summary>
    internal static (long Count, long SuccessCount, long TotalTicks) GetDecodeStats()
        => (Interlocked.Read(ref _decodeCount),
            Interlocked.Read(ref _decodeSuccessCount),
            Interlocked.Read(ref _decodeTotalTicks));

    /// <summary>Resets decode metrics (test support).</summary>
    internal static void ResetDecodeStats()
    {
        Interlocked.Exchange(ref _decodeCount, 0);
        Interlocked.Exchange(ref _decodeSuccessCount, 0);
        Interlocked.Exchange(ref _decodeTotalTicks, 0);
    }

    internal record struct RawFileCacheEntry(byte[] Data, IthmbVariantProfile Profile, int FrameCount, int FrameSize)
    {
        public long LastAccess { get; set; }
    }
}
