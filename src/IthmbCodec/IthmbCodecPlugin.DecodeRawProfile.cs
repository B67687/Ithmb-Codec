// Raw-profile decoding for .ithmb files: dispatches to the appropriate pixel-format
// decoder (Rgb565, Rgb555, Yuv422 variants, Ycbcr420), applies post-decode rotation
// and crop, and manages the pixel-buffer lifecycle. Separated from the decode pipeline
// for independent compilation and file-size discipline.

using System.Collections.Concurrent;
using System.Diagnostics;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using ImageGlass.SDK.Plugins;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
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
        catch (Exception) when (!Debugger.IsAttached)
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
