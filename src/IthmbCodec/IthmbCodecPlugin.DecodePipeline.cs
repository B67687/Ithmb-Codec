// Core decode pipeline for .ithmb files: orchestrates JPEG detection, raw profile
// decode fallback, multi-frame caching, and the DecodeRawProfile dispatch.
// Separated from plugin ABI glue for independent AOT compilation.

using System.Buffers.Binary;
using System.Buffers;
using System.Diagnostics;
using System.Threading;
using System.IO;
using System.Runtime.InteropServices;
using System.Runtime.CompilerServices;
using ImageGlass.SDK.Plugins;
using static IthmbCodec.PhotoDb.PhotoDb;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{

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
        string path = "";
        try
        {
            if (filePath.Data == null || filePath.Length <= 0) return IGStatus.InvalidArg;
            path = new string(filePath.Data, 0, filePath.Length);
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

            // Log decode start for lifecycle observability
            Log(4, $"ITHMB|Decode|START|{Path.GetFileName(path)}|{fileSize}bytes");


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
            bool wasRealAttempt = ticks >= Stopwatch.Frequency / 1000;
            if (wasRealAttempt)
                Interlocked.Increment(ref _decodeSuccessCount);

            // Log decode lifecycle outcome with timing
            if (path.Length > 0)
            {
                double ms = ticks * 1000.0 / Stopwatch.Frequency;
                string status = wasRealAttempt ? "OK" : "EARLY";
                Log(4, $"ITHMB|Decode|END|{Path.GetFileName(path)}|{status}|{ms:F1}ms");
            }

            // Periodic stats summary every 100 decode attempts
            long count = Interlocked.Read(ref _decodeCount);
            if (count > 0 && count % 100 == 0)
            {
                long success = Interlocked.Read(ref _decodeSuccessCount);
                long totalTicks2 = Interlocked.Read(ref _decodeTotalTicks);
                double avgMs = count > 0
                    ? totalTicks2 * 1000.0 / Stopwatch.Frequency / count
                    : 0;
                Log(4, $"ITHMB|Stats|count={count}|success={success}|fail={count - success}|avg_ms={avgMs:F1}");
            }
        }
    }

}
