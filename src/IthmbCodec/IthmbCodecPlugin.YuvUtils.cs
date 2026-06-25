// Shared YUV→RGB conversion utilities for .ithmb raw profile decoders.
using System.Runtime.CompilerServices;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // ---------- YUV→RGB conversion ----------
    // Constants YuvRCoef/YuvGCoefCb/YuvGCoefCr/YuvBCoef defined in Rgb565Rgb555.cs

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    internal static void WriteYuvPixel(byte* pDst, int luma, int cb, int cr)
    {
        int r = Clamp(luma + ((YuvRCoef * cr) >> 8));
        int g = Clamp(luma - ((YuvGCoefCb * cb) >> 8) - ((YuvGCoefCr * cr) >> 8));
        int b = Clamp(luma + ((YuvBCoef * cb) >> 8));
        pDst[0] = (byte)b; pDst[1] = (byte)g;
        pDst[2] = (byte)r; pDst[3] = 255;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    internal static int Clamp(int v) => v < 0 ? 0 : v > 255 ? 255 : v;
}
