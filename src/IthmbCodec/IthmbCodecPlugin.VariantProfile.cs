// SPDX-License-Identifier: MIT
// Raw profile enums and variant profile record struct

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // ------------------------------ Raw profile enums ------------------------------
    internal enum IthmbEncoding { Rgb565, Rgb555, ReorderedRgb555, Yuv422, Ycbcr420, Jpeg }

    /// <summary>Raw profile for F-prefix .ithmb files (single image, no container).</summary>
    /// <param name="SwapChromaPlanes">If true, swaps Cb/Cr order in YCbCr 4:2:0 (some iPod variants).</param>
    /// <param name="ClChroma">Per-pixel 4-bit nibble chroma (Keith CL, not CLCL).</param>
    /// <param name="Rotation">Clockwise rotation in degrees (0, 90, 180, 270). Applied post-decode to BGRA output.</param>
    /// <param name="CropX">X offset of visible region within decoded frame (0 = no crop).</param>
    /// <param name="CropY">Y offset of visible region within decoded frame (0 = no crop).</param>
    /// <param name="CropWidth">Width of visible region (0 = no crop, uses full Width).</param>
    /// <param name="CropHeight">Height of visible region (0 = no crop, uses full Height).</param>
    /// <remarks>
    /// Crop fields support centered-padding photo formats where the visible image is
    /// smaller than the stored frame (e.g., format 1007 480×864 may have centered
    /// padding). When CropWidth/CropHeight are non-zero, the decoder crops the BGRA
    /// output to the specified region after decode and rotation. This avoids the
    /// black-border artifact visible in some photo formats.
    /// Based on iOpenPod's _crop_visible_region approach (48-profile analysis).
    /// </remarks>
    internal readonly record struct IthmbVariantProfile(
        int Prefix, int Width, int Height, IthmbEncoding Encoding,
        int FrameByteLength,
        bool SwapsDimensions = false, bool LittleEndian = true,
        bool IsPadded = false, bool IsInterlaced = false,
        bool ClclChroma = false,
        bool SwapChromaPlanes = false, bool ClChroma = false,
        bool SwapRgbChannels = false,
        int Rotation = 0,
        int CropX = 0, int CropY = 0,
        int CropWidth = 0, int CropHeight = 0, int SlotSize = 0,
        bool UseMhniDimensions = false,
        IthmbEncoding[]? FallbackEncodings = null);
}
