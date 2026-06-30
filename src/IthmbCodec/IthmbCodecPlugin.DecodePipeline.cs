// Core decode pipeline for .ithmb files: orchestrates JPEG detection, raw profile
// decode fallback, multi-frame caching, and the DecodeRawProfile dispatch.
// Separated from plugin ABI glue for independent AOT compilation.

using System.Buffers.Binary;
using System.Buffers;
using System.Diagnostics;
using System.Collections.Concurrent;
using System.Threading;
using System.IO;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using ImageGlass.SDK.Plugins;
using static IthmbCodec.PhotoDb.PhotoDb;

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

    // ------------------------------ Core decode pipeline ------------------------------

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static IGStatus CodecDecodeStaticRaster(IGStringRef filePath, int frameIndex,
        IGPixelBuffer* outBuf, void* cancellation)
    {
        if (outBuf == null) return IGStatus.InvalidArg;
        *outBuf = default;
        if (frameIndex < 0) return IGStatus.InvalidArg;
        IGImageInfo info = default;
        return DecodeInternal(filePath, cancellation, &info, outBuf, frameIndex);
    }

    internal static IGStatus DecodeInternal(IGStringRef filePath, void* cancellation,
        IGImageInfo* outInfo, IGPixelBuffer* outBuf, int frameIndex = 0)
    {
        Interlocked.Increment(ref _decodeCount);
        var sw = Stopwatch.StartNew();
        try
        {
        if (filePath.Data == null || filePath.Length <= 0) return IGStatus.InvalidArg;
        var path = new string(filePath.Data, 0, filePath.Length);
        if (path.Contains('\0')) return IGStatus.InvalidArg;
        if (IsCanceled(cancellation)) return IGStatus.Canceled;

        // Load external profiles on first decode (deferred from init to avoid I/O in GetApi)
        if (!_profilesLoaded)
        {
            lock (_initLock)
            {
                if (!_profilesLoaded) { LoadExternalProfiles(); _profilesLoaded = true; }
            }
        }

        // Check file size before reading
        long fileSize;
        try { fileSize = new FileInfo(path).Length; }
        catch (Exception) { return IGStatus.IoError; }
        if (fileSize > MaxDecodeFileSize)
        {
            Log(4, $"ITHMB: '{Path.GetFileName(path)}' file too large ({fileSize} bytes)");
            return IGStatus.DecodeFailed;
        }

        // Two-phase header probe: read 512 KB first for JPEG scan;
        // extend to full peek buffer (4 MB) only if no SOI found.
        // This avoids reading 4 MB for common small thumbnails (~500 KB).

        // Network filesystem note: FileStream reads block the calling thread.
        // On a network share, an unresponsive server could hang the plugin
        // indefinitely. A future enhancement may add a CancellationToken-based
        // timeout wrapper around the file read.

        try
        {
            using var fs = new FileStream(path, FileMode.Open, FileAccess.Read);
            int jpegOffset = 0, jpegLength = 0;
            bool foundJpeg;
            int peekSize = (int)Math.Min(fileSize, 512 * 1024);

            // Phase 1: probe first 512 KB for JPEG SOI
            byte[] probeBuffer = ArrayPool<byte>.Shared.Rent(peekSize);
            try
            {
                fs.ReadExactly(probeBuffer, 0, peekSize);
                foundJpeg = TryFindJpegSlice(probeBuffer.AsSpan(0, peekSize), out jpegOffset, out jpegLength, cancellation);
            }
            finally
            {
                ArrayPool<byte>.Shared.Return(probeBuffer);
            }

            // Phase 2: if not found and file is larger, extend to full peek buffer
            if (!foundJpeg && fileSize > peekSize)
            {
                peekSize = (int)Math.Min(fileSize, PeekBufferSize);
                byte[] fullBuffer = ArrayPool<byte>.Shared.Rent(peekSize);
                try
                {
                    fs.Seek(0, SeekOrigin.Begin);
                    fs.ReadExactly(fullBuffer, 0, peekSize);
                    foundJpeg = TryFindJpegSlice(fullBuffer.AsSpan(0, peekSize), out jpegOffset, out jpegLength, cancellation);
                }
                finally
                {
                    ArrayPool<byte>.Shared.Return(fullBuffer);
                }
            }

            if (foundJpeg)
            {
                // If JPEG EOI extends beyond peek buffer, read tail to find true EOI
                if (jpegOffset + jpegLength >= peekSize && fileSize > peekSize)
                {
                    long tailSize = fileSize - (jpegOffset + 2);
                    byte[] tail = new byte[tailSize > 0 ? (int)Math.Min(tailSize, MaxDecodeFileSize) : 0];
                    if (tail.Length > 0)
                    {
                        fs.Seek(jpegOffset + 2, SeekOrigin.Begin);
                        fs.ReadExactly(tail, 0, tail.Length);
                        int eoiRel = tail.AsSpan().IndexOf(JpegEoiMarker);
                        if (eoiRel >= 0)
                        {
                            // Use the actual file position, not the peek offset
                            long actualEoiPos = jpegOffset + 2L + eoiRel + 2;
                            jpegLength = (int)(actualEoiPos - jpegOffset);
                        }
                        else
                        {
                            // No EOI found — use rest of file
                            jpegLength = (int)(fileSize - jpegOffset);
                        }
                    }
                }

                // Always read the JPEG slice from the FileStream (not the peek buffer)
                byte[] jpegSlice = new byte[jpegLength];
                fs.Seek(jpegOffset, SeekOrigin.Begin);
                int bytesRead = fs.ReadAtLeast(jpegSlice, jpegLength, throwOnEndOfStream: false);
                if (bytesRead < jpegLength) { Log(4, $"ITHMB: '{Path.GetFileName(path)}' truncated JPEG read ({bytesRead}/{jpegLength})"); return IGStatus.DecodeFailed; }
                // fileSize ≤ MaxDecodeFileSize, safe for int
                return DecodeJpegSlice(jpegSlice, jpegLength, (int)fileSize,
                    cancellation, outInfo, outBuf);
            }

            // No embedded JPEG found in peek buffer — read full file for raw profile fallback
            byte[] fileBytes = new byte[(int)fileSize];
            fs.Seek(0, SeekOrigin.Begin);
            fs.ReadExactly(fileBytes, 0, (int)fileSize);

            // PhotoDB/ArtworkDB container check — iPod thumbnail databases embed raw .ithmb blobs
            if (fileBytes.Length >= 4 && CanOpenPhotoDb(fileBytes))
            {
                if (!TryParsePhotoDb(fileBytes, out var pdEntries, out var pdFrameCount))
                {
                    Log(4, $"ITHMB: '{Path.GetFileName(path)}' PhotoDB parse failed");
                    return IGStatus.DecodeFailed;
                }

                if (frameIndex >= pdFrameCount)
                {
                    FillImageInfo(outInfo, 0, 0, hasAlpha: 0, orientation: 1,
                        fileSize: (int)fileSize, frameCount: pdFrameCount);
                    return IGStatus.InvalidArg;
                }

                var (pdFormatId, pdRawData, _, _, pdW, pdH) = pdEntries[frameIndex];
                if (!TryResolveProfile(pdFormatId, pdRawData.AsSpan(), out var pdProfile))
                {
                    if (pdRawData.Length >= 2 && pdRawData[0] == 0xFF && pdRawData[1] == 0xD8)
                    {
                        return DecodeJpegSlice(pdRawData, pdRawData.Length, (int)fileSize,
                            cancellation, outInfo, outBuf);
                    }
                    Log(4, $"ITHMB: '{Path.GetFileName(path)}' PhotoDB format_id {pdFormatId} has no decoder profile");
                    return IGStatus.DecodeFailed;
                }

                // Construct synthetic .ithmb buffer: 4-byte placeholder prefix + raw pixel data.
                // DecodeRawProfile expects frameStart = 4 + frameIndex * frameSize.
                // With prefix_bytes|raw_data and frameIndex=0, frameStart=4 skips the prefix.
                byte[] synthetic = new byte[4 + pdRawData.Length];
                pdRawData.CopyTo(synthetic, 4);

                int useW = pdProfile.UseMhniDimensions && pdW > 0 ? pdW : pdProfile.Width;
                int useH = pdProfile.UseMhniDimensions && pdH > 0 ? pdH : pdProfile.Height;
                if (pdProfile.SwapsDimensions) (useW, useH) = (useH, useW);

                FillImageInfo(outInfo, useW, useH, hasAlpha: 0, orientation: 1,
                    fileSize: synthetic.Length, frameCount: pdFrameCount);
                if (outBuf == null) return IGStatus.OK;

                return DecodeRawProfile(synthetic, pdProfile, cancellation, outInfo, outBuf, frameIndex: 0, overrideW: useW, overrideH: useH);
            }

            // Read the 4-byte big-endian prefix. This is either a format_id matched against
            // KnownProfiles (for F-prefix raw decodes) or ignored for T-prefix JPEG blobs.
            // No 'F'/'T' byte guard is needed — the KnownProfiles lookup and JPEG carving
            // fallback below already handle unknown/corrupted files correctly, and the guard
            // was blocking our own encoder output (format IDs < 65536 have first byte 0x00).
            if (fileBytes.Length < 4) return IGStatus.DecodeFailed;
            int prefix = BinaryPrimitives.ReadInt32BigEndian(fileBytes.AsSpan(0, 4));
            if (TryResolveProfile(prefix, fileBytes.AsSpan(4), out var profile))
            {
                // Check cache first (populated by a previous frameIndex or metadata call)
                if (TryGetCachedFile(path, out var cached))
                {
                    if (frameIndex >= cached.FrameCount) return IGStatus.InvalidArg;
                    return DecodeRawProfile(cached.Data, cached.Profile, cancellation, outInfo, outBuf, frameIndex);
                }

                // First time: compute frame count and cache the raw data.
                // F-prefix .ithmb files can contain multiple concatenated raw frames.
                int frameSize = profile.FrameByteLength;
                if (frameSize <= 0)
                {
                    Log(4, $"ITHMB: '{Path.GetFileName(path)}' profile {profile.Prefix} has invalid frameSize={frameSize}");
                    return IGStatus.DecodeFailed;
                }
                int dataLen = fileBytes.Length - 4;
                int frameCount = frameSize > 0 ? dataLen / frameSize : 1;
                if (frameCount < 1) frameCount = 1;

                // Atomically store via bounded LRU setter (evicts when MaxCachedPaths exceeded).
                SetCachedFile(path, fileBytes, profile, frameCount, frameSize);

                if (frameIndex >= frameCount) return IGStatus.InvalidArg;
                return DecodeRawProfile(fileBytes, profile, cancellation, outInfo, outBuf, frameIndex);
            }

            // Early bailout: files >8 MB with an unknown prefix are unlikely to be valid
            // .ithmb files — skip the expensive full-file JPEG carving scan.
            if (fileBytes.Length > MaxCarvingFileSize)
            {
                Log(4, $"ITHMB: '{Path.GetFileName(path)}' file too large ({fileBytes.Length} bytes) for JPEG carving, unknown prefix {prefix}");
                return IGStatus.DecodeFailed;
            }

            // Unknown prefix — try JPEG carving on the full file before giving up.
            // Many .ithmb files from newer devices embed JPEGs regardless of prefix,
            // and the JPEG may start beyond the 4 MB peek buffer or lack standard
            // JFIF/Exif markers in the first scan window. File Juicer uses this
            // byte-level carving approach successfully for unknown variants.
            Log(4, $"ITHMB: '{Path.GetFileName(path)}' unknown prefix {prefix}, trying JPEG carving fallback");
            if (TryFindJpegSlice(fileBytes, out var carveOffset, out var carveLength, cancellation))
            {
                Log(4, $"ITHMB: '{Path.GetFileName(path)}' JPEG carving found slice at offset {carveOffset}, length {carveLength}");
                var carveSlice = fileBytes.AsSpan(carveOffset, carveLength).ToArray();
                return DecodeJpegSlice(carveSlice, carveLength, (int)fileSize,
                    cancellation, outInfo, outBuf);
            }

            Log(4, $"ITHMB: '{Path.GetFileName(path)}' no embedded JPEG or known profile (prefix {prefix})");
            return IGStatus.DecodeFailed;
        }
        catch (IOException ex) { Log(4, $"ITHMB: read failed '{path}' ({ex.Message})"); return IGStatus.IoError; }
        catch (Exception ex) { Log(4, $"ITHMB: unexpected error reading '{path}' ({ex.Message})"); return IGStatus.Internal; }
        }
        finally
        {
            long ticks = sw.ElapsedTicks;
            Interlocked.Add(ref _decodeTotalTicks, ticks);
            // Successful decode: ran long enough (>=1ms) to be a real attempt,
            // not an early-return error path (NUL, canceled, invalid frameIndex).
            if (ticks >= Stopwatch.Frequency / 1000)
                Interlocked.Increment(ref _decodeSuccessCount);
        }
    }

    // ------------------------------ Raw profile decoding ------------------------------
    internal static IGStatus DecodeRawProfile(byte[] data, IthmbVariantProfile profile,
        void* cancellation, IGImageInfo* outInfo, IGPixelBuffer* outBuf, int frameIndex = 0, int overrideW = 0, int overrideH = 0)
    {
        int w = overrideW > 0 ? overrideW : profile.Width;
        int h = overrideH > 0 ? overrideH : profile.Height;
        // SlotSize overrides FrameByteLength for frame slicing when set (padded profiles
        // with fixed slot boundaries, e.g., iPod Touch cover art at 8192/16384 byte slots).
        int frameSize = profile.SlotSize > 0 ? profile.SlotSize : profile.FrameByteLength;

        // Compute frame count for multi-image support.
        // F-prefix .ithmb files can contain multiple concatenated raw frames,
        // each exactly frameSize bytes. Slot/padding mechanism verified by libgpod
        // (itdb_device.c padding fields); multi-frame concatenation confirmed by
        // Keith's iPod Photo Reader, ithmbrdr, and iOpenPod.
        int dataAfterPrefix = data.Length - 4;
        int frameCount = frameSize > 0 ? dataAfterPrefix / frameSize : 1;
        if (frameCount < 1) frameCount = 1;

        // Validate frameIndex
        if (frameIndex < 0 || frameIndex >= frameCount)
        {
            Log(4, $"ITHMB: frameIndex {frameIndex} out of range (0-{frameCount - 1})");
            return IGStatus.InvalidArg;
        }

        // Compute the minimum size we need. For padded profiles, the valid pixel data
        // may be smaller than the slot size (padding was added by the device).
        // For non-padded profiles, apply TrailingPaddingTolerance to handle device
        // alignment quirks where the encoder wrote fewer bytes than expected.
        int requiredSize = profile.IsPadded
            ? profile.FrameByteLength  // actual pixel data size (smaller than slot)
            : frameSize;                // same as slot for non-padded
        if (requiredSize < 0) requiredSize = int.MaxValue;

        // Slice to the requested frame
        int frameStart = 4 + frameIndex * frameSize;
        int actualDataLen = data.Length - frameStart;
        if (actualDataLen < 0) { Log(4, "ITHMB: frame offset past file end"); return IGStatus.DecodeFailed; }
        // Reject if file is shorter than requiredSize minus tolerance.
        // TrailingPaddingTolerance is inclusive: a file exactly (requiredSize - tolerance)
        // bytes is accepted (device alignment quirk, iOpenPod trailing-trim pattern).
        if (actualDataLen < requiredSize - TrailingPaddingTolerance)
        {
            Log(4, $"ITHMB: raw file too small ({actualDataLen} < {requiredSize})");
            return IGStatus.DecodeFailed;
        }

        int fileSize = data.Length; // actual file bytes read (available before FillImageInfo)
        FillImageInfo(outInfo, w, h, hasAlpha: 0, orientation: 1, fileSize: fileSize, frameCount: frameCount);

        if (outBuf == null) return IGStatus.OK;
        if (IsCanceled(cancellation)) return IGStatus.Canceled;

        var allocStatus = AllocateBgraBuffer(w, h, out var stride, out var pixels);
        if (allocStatus != IGStatus.OK) return allocStatus;

        try
        {
            // Slice to frame boundary (multi-frame .ithmb files concatenate frames)
            int rawLen = Math.Min(frameSize, data.Length - frameStart);
            var raw = data.AsSpan(frameStart, rawLen);
            // For padded profiles, trim to the valid pixel data portion
            if (profile.IsPadded)
            {
                int validSize = profile.Encoding switch
                {
                    IthmbEncoding.Ycbcr420 => w * h + ((w + 1) / 2) * ((h + 1) / 2) * 2,
                    _ => w * h * 2 // RGB565/RGB555/YUV422 are all 2 Bpp
                };
                if (raw.Length >= validSize) raw = raw[..validSize];
                else if (raw.Length < validSize && raw.Length >= validSize - TrailingPaddingTolerance)
                {
                    // File is within tolerance of validSize (e.g., slightly undersized padded slot).
                    // Zero-pad to validSize so the decoder can read the expected number of bytes.
                    var padded = new byte[validSize];
                    raw.CopyTo(padded);
                    raw = padded;
                }
            }
            // For non-padded profiles, handle trailing alignment tolerance:
            // if raw is slightly shorter than frameSize (within TrailingPaddingTolerance),
            // zero-pad it to frameSize so the decoder can read the expected number of bytes.
            else if (raw.Length < frameSize && raw.Length >= frameSize - TrailingPaddingTolerance)
            {
                var padded = new byte[frameSize];
                raw.CopyTo(padded);
                raw = padded;
            }
            bool ok = TryDecode(raw, pixels, w, h, profile.Encoding, profile);
            if (!ok && profile.FallbackEncodings != null)
            {
                foreach (var fallbackEnc in profile.FallbackEncodings)
                {
                    ok = TryDecode(raw, pixels, w, h, fallbackEnc, profile);
                    if (ok) break;
                }
            }
            if (!ok)
            {
                // JPEG fallback: if profile declares JPEG as a fallback encoding,
                // check if the raw frame data contains JPEG markers (0xFF 0xD8).
                // This handles format 1081 where libgpod says JPEG while iOpenPod says RGB565.
                if (profile.FallbackEncodings != null
                    && Array.IndexOf(profile.FallbackEncodings, IthmbEncoding.Jpeg) >= 0
                    && raw.Length >= 2 && raw[0] == 0xFF && raw[1] == 0xD8)
                {
                    NativeMemory.Free(pixels);
                    byte[] jpegData = raw.ToArray();
                    return DecodeJpegSlice(jpegData, jpegData.Length, jpegData.Length,
                        cancellation, outInfo, outBuf);
                }
                NativeMemory.Free(pixels);
                return IGStatus.DecodeFailed;
            }

            // Apply post-decode rotation (speculative, for profiles with Rotation≠0)
            if (profile.Rotation != 0 && w > 0 && h > 0)
            {
                RotateBgra(pixels, ref w, ref h, profile.Rotation);
                stride = (ulong)w * 4UL;
            }

            // Apply post-decode crop for centered-padding photo formats.
            // Crop is applied AFTER rotation so the crop region references the final orientation.
            // When CropWidth/CropHeight are non-zero, copy the visible region into a new
            // buffer and free the full-frame buffer. Based on iOpenPod's _crop_visible_region.
            if (profile.CropWidth > 0 && profile.CropHeight > 0 &&
                profile.CropX >= 0 && profile.CropY >= 0 &&
                (long)profile.CropX + profile.CropWidth <= w &&
                (long)profile.CropY + profile.CropHeight <= h)
            {
                int cropW = profile.CropWidth, cropH = profile.CropHeight;
                byte* cropped = (byte*)NativeMemory.AllocZeroed((nuint)(cropW * 4 * cropH));
                if (cropped != null)
                {
                    for (int y = 0; y < cropH; y++)
                    {
                        int srcOff = ((profile.CropY + y) * w + profile.CropX) * 4;
                        int dstOff = y * cropW * 4;
                        NativeMemory.Copy(pixels + srcOff, cropped + dstOff, (nuint)(cropW * 4));
                    }
                    NativeMemory.Free(pixels);
                    _liveBuffers.TryRemove((nint)pixels, out _);
                    pixels = cropped;
                    w = cropW;
                    h = cropH;
                    stride = (ulong)cropW * 4UL;
                }
            }
        }
        catch (Exception) when (!System.Diagnostics.Debugger.IsAttached)
#pragma warning disable CS0168 // Deliberate catch-all: Native AOT plugin error boundary — 
        // any exception from unsafe decode must be caught to avoid crashing the host.
        // Specific exception types (AccessViolationException, OutOfMemoryException)
        // are unreliable in Native AOT trimming scenarios.
#pragma warning restore CS0168
        {
            NativeMemory.Free(pixels);
            return IGStatus.Internal;
        }

        _liveBuffers[(nint)pixels] = 0;
        outBuf->Data = pixels;
        outBuf->Width = w;
        outBuf->Height = h;
        outBuf->Stride = (int)stride;
        outBuf->PixelFormat = (int)IGPixelFormat.Bgra8Unorm;
        return IGStatus.OK;
    }

    /// <summary>Tries decoding raw data with the given encoding. Returns false on failure.</summary>
    [MethodImpl(MethodImplOptions.AggressiveOptimization)]
    private static bool TryDecode(ReadOnlySpan<byte> raw, byte* pixels, int w, int h, IthmbEncoding enc, IthmbVariantProfile profile)
    {
        return enc switch
        {
            IthmbEncoding.Rgb565 => DecodeRgb565(raw, pixels, w, h, profile.LittleEndian),
            IthmbEncoding.Rgb555 => DecodeRgb555(raw, pixels, w, h, profile.LittleEndian, profile.SwapRgbChannels),
            IthmbEncoding.ReorderedRgb555 => DecodeReorderedRgb555(raw, pixels, w, h, profile.LittleEndian),
            IthmbEncoding.Yuv422 => profile.ClChroma
                ? DecodeYuv422Cl(raw, pixels, w, h)
                : profile.ClclChroma
                ? DecodeYuv422Clcl(raw, pixels, w, h)
                : profile.IsInterlaced
                ? DecodeYuv422Interlaced(raw, pixels, w, h)
                : DecodeYuv422(raw, pixels, w, h),
            IthmbEncoding.Ycbcr420 => DecodeYcbcr420(raw, pixels, w, h, profile.SwapChromaPlanes),
            _ => false,
        };
    }
}
