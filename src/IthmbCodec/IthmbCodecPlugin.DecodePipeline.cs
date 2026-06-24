// Core decode pipeline for .ithmb files: orchestrates JPEG detection, raw profile
// decode fallback, multi-frame caching, and the DecodeRawProfile dispatch.
// Separated from plugin ABI glue for independent AOT compilation.

using System.Collections.Concurrent;
using System.IO;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using ImageGlass.SDK.Plugins;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // Cache for multi-frame raw .ithmb files. Populated by the first DecodeInternal
    // call for a raw file, reused across subsequent frameIndex values without re-reading.
    // Read-once, decode-many: the ithmb file is read in full once and cached here.
    // Only the most recent file is kept (evicted when a different path is encountered).
    private static readonly ConcurrentDictionary<string, RawFileCacheEntry> _rawFileCache = new();

    private readonly struct RawFileCacheEntry(byte[] data, IthmbVariantProfile profile, int frameCount, int frameSize)
    {
        public readonly byte[] Data = data;
        public readonly IthmbVariantProfile Profile = profile;
        public readonly int FrameCount = frameCount;
        public readonly int FrameSize = frameSize;
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
        if (filePath.Data == null || filePath.Length <= 0) return IGStatus.InvalidArg;
        var path = new string(filePath.Data, 0, filePath.Length);
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
            Log(4, $"ITHMB: file too large ({fileSize} bytes)");
            return IGStatus.DecodeFailed;
        }

        // Read a header buffer for JPEG scan (4 MB or file size, whichever is smaller)
        // This avoids reading the entire file into memory for the common JPEG-embedded path.
        try
        {
            using var fs = new FileStream(path, FileMode.Open, FileAccess.Read);
            int peekSize = (int)Math.Min(fileSize, PeekBufferSize);
            byte[] peek = new byte[peekSize];
            fs.ReadExactly(peek, 0, peekSize);

            if (TryFindJpegSlice(peek, out var jpegOffset, out var jpegLength, cancellation))
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
                if (bytesRead < jpegLength) { Log(4, $"ITHMB: truncated JPEG read ({bytesRead}/{jpegLength})"); return IGStatus.DecodeFailed; }
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
                        Log(4, "ITHMB: PhotoDB parse failed");
                        return IGStatus.DecodeFailed;
                    }

                    if (frameIndex >= pdFrameCount)
                    {
                        FillImageInfo(outInfo, 0, 0, hasAlpha: 0, orientation: 1,
                            fileSize: (int)fileSize, frameCount: pdFrameCount);
                        return IGStatus.InvalidArg;
                    }

                    var (pdFormatId, pdRawData) = pdEntries[frameIndex];
                    if (!KnownProfiles.TryGetValue(pdFormatId, out var pdProfile))
                    {
                        Log(4, $"ITHMB: PhotoDB format_id {pdFormatId} has no decoder profile");
                        return IGStatus.DecodeFailed;
                    }

                    // Construct synthetic .ithmb buffer: 4-byte placeholder prefix + raw pixel data.
                    // DecodeRawProfile expects frameStart = 4 + frameIndex * frameSize.
                    // With prefix_bytes|raw_data and frameIndex=0, frameStart=4 skips the prefix.
                    byte[] synthetic = new byte[4 + pdRawData.Length];
                    pdRawData.CopyTo(synthetic, 4);

                    FillImageInfo(outInfo, pdProfile.Width, pdProfile.Height, hasAlpha: 0, orientation: 1,
                        fileSize: synthetic.Length, frameCount: pdFrameCount);
                    if (outBuf == null) return IGStatus.OK;

                    // frameIndex=0 for the synthetic buffer (it contains exactly one frame)
                    return DecodeRawProfile(synthetic, pdProfile, cancellation, outInfo, outBuf, frameIndex: 0);
                }

            // Read the 4-byte big-endian prefix. This is either a format_id matched against
            // KnownProfiles (for F-prefix raw decodes) or ignored for T-prefix JPEG blobs.
            // No 'F'/'T' byte guard is needed — the KnownProfiles lookup and JPEG carving
            // fallback below already handle unknown/corrupted files correctly, and the guard
            // was blocking our own encoder output (format IDs < 65536 have first byte 0x00).
            int prefix = ReadInt32BigEndian(fileBytes, 0);
            if (KnownProfiles.TryGetValue(prefix, out var profile))
            {
                // Check cache first (populated by a previous frameIndex or metadata call)
                if (_rawFileCache.TryGetValue(path, out var cached))
                {
                    if (frameIndex >= cached.FrameCount) return IGStatus.InvalidArg;
                    return DecodeRawProfile(cached.Data, cached.Profile, cancellation, outInfo, outBuf, frameIndex);
                }

                // First time: compute frame count and cache the raw data.
                // F-prefix .ithmb files can contain multiple concatenated raw frames.
                int frameSize = profile.FrameByteLength;
                int dataLen = fileBytes.Length - 4;
                int frameCount = frameSize > 0 ? dataLen / frameSize : 1;
                if (frameCount < 1) frameCount = 1;

                // Atomically store — ConcurrentDictionary indexer is thread-safe per-key.
                // No Clear(): a concurrent Clear()+Set() for another path could interleave
                // and overwrite our entry, losing that path's cached data.
                _rawFileCache[path] = new RawFileCacheEntry(fileBytes, profile, frameCount, frameSize);

                if (frameIndex >= frameCount) return IGStatus.InvalidArg;
                return DecodeRawProfile(fileBytes, profile, cancellation, outInfo, outBuf, frameIndex);
            }

                    // Unknown prefix — try JPEG carving on the full file before giving up.
                // Many .ithmb files from newer devices embed JPEGs regardless of prefix,
                // and the JPEG may start beyond the 4 MB peek buffer or lack standard
                // JFIF/Exif markers in the first scan window. File Juicer uses this
                // byte-level carving approach successfully for unknown variants.
                Log(4, $"ITHMB: '{Path.GetFileName(path)}' unknown prefix {prefix}, trying JPEG carving fallback");
                if (TryFindJpegSlice(fileBytes, out var carveOffset, out var carveLength, cancellation))
                {
                    Log(4, $"ITHMB: JPEG carving found slice at offset {carveOffset}, length {carveLength}");
                    return DecodeJpegSlice(fileBytes, carveLength, (int)fileSize,
                        cancellation, outInfo, outBuf);
                }

            Log(4, $"ITHMB: '{Path.GetFileName(path)}' no embedded JPEG or known profile (prefix {prefix})");
            return IGStatus.DecodeFailed;
        }
        catch (IOException ex) { Log(4, $"ITHMB: read failed '{path}' ({ex.Message})"); return IGStatus.IoError; }
        catch (Exception ex) { Log(4, $"ITHMB: unexpected error reading '{path}' ({ex.Message})"); return IGStatus.Internal; }
    }

    // ------------------------------ Raw profile decoding ------------------------------
    internal static IGStatus DecodeRawProfile(byte[] data, IthmbVariantProfile profile,
        void* cancellation, IGImageInfo* outInfo, IGPixelBuffer* outBuf, int frameIndex = 0)
    {
        int w = profile.Width, h = profile.Height;
        if (profile.SwapsDimensions) (w, h) = (h, w);
        int frameSize = profile.FrameByteLength;

        // Compute frame count for multi-image support.
        // F-prefix .ithmb files can contain multiple concatenated raw frames,
        // each exactly FrameByteLength bytes (confirmed by Keith's iPod Photo Reader,
        // ithmbrdr, libgpod, and iOpenPod).
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
        // may be smaller than FrameByteLength (padding was added by the device).
        // For non-padded profiles, apply TrailingPaddingTolerance to handle device
        // alignment quirks where the encoder wrote fewer bytes than expected.
        int requiredSize = profile.IsPadded
            ? Math.Min(frameSize, (int)Math.Min((long)w * h + (long)((w + 1) / 2) * ((h + 1) / 2) * 2, int.MaxValue))
            : frameSize;
        if (requiredSize < 0) requiredSize = int.MaxValue;

        // Slice to the requested frame
        int frameStart = 4 + frameIndex * frameSize;
        int actualDataLen = data.Length - frameStart;
        if (actualDataLen < 0) { Log(4, "ITHMB: frame offset past file end"); return IGStatus.DecodeFailed; }
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
                if (raw.Length > validSize) raw = raw[..validSize];
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
            bool ok = profile.Encoding switch
            {
                IthmbEncoding.Rgb565 => DecodeRgb565(raw, pixels, w, h, profile.LittleEndian),
                IthmbEncoding.Rgb555 => DecodeRgb555(raw, pixels, w, h, profile.LittleEndian, profile.SwapRgbChannels),
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
            if (!ok)
            {
                NativeMemory.Free(pixels);
                return IGStatus.DecodeFailed;
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

        _liveBuffers[(nint)pixels] = 0;
        outBuf->Data = pixels;
        outBuf->Width = w;
        outBuf->Height = h;
        outBuf->Stride = (int)stride;
        outBuf->PixelFormat = (int)IGPixelFormat.Bgra8Unorm;
        return IGStatus.OK;
    }
}
