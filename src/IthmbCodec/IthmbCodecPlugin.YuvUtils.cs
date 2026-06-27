// Shared YUV-to-RGB conversion utilities for .ithmb raw profile decoders.
using System.Runtime.CompilerServices;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // ---------- YUV-to-RGB conversion ----------

    // BT.601 YUV-to-RGB conversion coefficients (ITU-R BT.601-7)
    internal const int YuvRCoef = 359;   //  1.402  (Cr contribution to R)
    internal const int YuvGCoefCb = 88;  // -0.344  (Cb contribution to G)
    internal const int YuvGCoefCr = 183; // -0.714  (Cr contribution to G)
    internal const int YuvBCoef = 454;   //  1.772  (Cb contribution to B)

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
