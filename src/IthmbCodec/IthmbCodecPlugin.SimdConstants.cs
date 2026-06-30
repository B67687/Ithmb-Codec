// SIMD constants centralized from across decoder files.
// Each decoder method references SimdConstants.X instead of declaring its own local copies.
// Shuffle masks are static readonly fields (no static init ordering concerns).
// Coefficient vectors are static get-only properties (avoid static init ordering with YuvRCoef etc.).
using System.Runtime.Intrinsics;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    /// <summary>SIMD constants shared across decoder methods.</summary>
    internal static class SimdConstants
    {
        // ---- Shuffle masks (pshufb / VectorTableLookup) ----

        /// <summary>pshufb mask: extract Y from UYVY bytes (positions 1,3,5,7,9,11,13,15).</summary>
        internal static readonly Vector128<byte> ShufY = Vector128.Create(
            (byte)1, 0x80, 3, 0x80, 5, 0x80, 7, 0x80, 9, 0x80, 11, 0x80, 13, 0x80, 15, 0x80);

        /// <summary>pshufb mask: extract U from UYVY (positions 0,4,8,12), replicate to adjacent 16-bit lanes.</summary>
        internal static readonly Vector128<byte> ShufU = Vector128.Create(
            (byte)0, 0x80, 0, 0x80, 4, 0x80, 4, 0x80, 8, 0x80, 8, 0x80, 12, 0x80, 12, 0x80);

        /// <summary>pshufb mask: extract V from UYVY (positions 2,6,10,14), replicate to adjacent 16-bit lanes.</summary>
        internal static readonly Vector128<byte> ShufV = Vector128.Create(
            (byte)2, 0x80, 2, 0x80, 6, 0x80, 6, 0x80, 10, 0x80, 10, 0x80, 14, 0x80, 14, 0x80);

        /// <summary>pshufb mask: extract Y from CL bytes (odd positions 1,3,5,7,9,11,13,15).</summary>
        internal static readonly Vector128<byte> ClShufY = Vector128.Create(
            (byte)1, 0x80, 3, 0x80, 5, 0x80, 7, 0x80, 9, 0x80, 11, 0x80, 13, 0x80, 15, 0x80);

        /// <summary>pshufb mask: extract CbCr bytes from CL (even positions 0,2,4,6,8,10,12,14).</summary>
        internal static readonly Vector128<byte> ClShufC = Vector128.Create(
            (byte)0, 0x80, 2, 0x80, 4, 0x80, 6, 0x80, 8, 0x80, 10, 0x80, 12, 0x80, 14, 0x80);

        /// <summary>pshufb mask: extract Y from CLCL bytes (odd positions 1,3,5,7,9,11,13,15).</summary>
        internal static readonly Vector128<byte> ClclShufY = Vector128.Create(
            (byte)1, 0x80, 3, 0x80, 5, 0x80, 7, 0x80, 9, 0x80, 11, 0x80, 13, 0x80, 15, 0x80);

        /// <summary>pshufb mask: extract CbCr bytes from CLCL (even positions 0,2,4,6,8,10,12,14), replicate to adjacent 16-bit lanes.</summary>
        internal static readonly Vector128<byte> ClclShufC = Vector128.Create(
            (byte)0, 0x80, 0, 0x80, 4, 0x80, 4, 0x80, 8, 0x80, 8, 0x80, 12, 0x80, 12, 0x80);

        // ---- Coefficient / constant vectors (get-only properties avoid static init ordering) ----

        /// <summary>Vector128&lt;int&gt;.Zero for branchless clamping.</summary>
        internal static Vector128<int> ZeroI => Vector128<int>.Zero;

        /// <summary>Vector128(255) for clamping to byte range.</summary>
        internal static Vector128<int> Max255 => Vector128.Create(255);

        /// <summary>Vector128(255 &lt;&lt; 24) for alpha channel (0xFF000000).</summary>
        internal static Vector128<int> Alpha => Vector128.Create(255 << 24);

        /// <summary>BT.601 Cr contribution to R: Vector128(YuvRCoef) = Vector128(359).</summary>
        internal static Vector128<int> RCoef => Vector128.Create(YuvRCoef);

        /// <summary>BT.601 Cb contribution to G: Vector128(YuvGCoefCb) = Vector128(88).</summary>
        internal static Vector128<int> GCoefCb => Vector128.Create(YuvGCoefCb);

        /// <summary>BT.601 Cr contribution to G: Vector128(YuvGCoefCr) = Vector128(183).</summary>
        internal static Vector128<int> GCoefCr => Vector128.Create(YuvGCoefCr);

        /// <summary>BT.601 Cb contribution to B: Vector128(YuvBCoef) = Vector128(454).</summary>
        internal static Vector128<int> BCoef => Vector128.Create(YuvBCoef);
    }
}
