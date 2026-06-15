// Encoder that generates valid F-prefix .ithmb raw-format files from BGRA pixel data.
// This is the inverse of the decoders in IthmbCodecPlugin.Decoding.cs.
// Used for synthetic test file generation and roundtrip verification.

using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // BT.601 fixed-point coefficients (same as Decoding.cs, used for YUV encoding)
    // Forward transform: BGRA → YCbCr (full-range JPEG variant)
    // Y  =  0.299*R + 0.587*G + 0.114*B
    // Cb = -0.169*R - 0.331*G + 0.500*B + 128
    // Cr =  0.500*R - 0.419*G - 0.081*B + 128

    // ---- RGB565 encoder ----
    internal static byte[] EncodeRgb565(ReadOnlySpan<byte> bgra, int w, int h, bool bigEndian)
    {
        int pixelCount = w * h;
        var result = new byte[pixelCount * 2];
        for (int i = 0; i < pixelCount; i++)
        {
            int srcOff = i * 4;
            int r = bgra[srcOff + 2] >> 3;  // R: top 5 bits
            int g = bgra[srcOff + 1] >> 2;  // G: top 6 bits
            int b = bgra[srcOff] >> 3;       // B: top 5 bits
            ushort pixel = (ushort)((r << 11) | (g << 5) | b);

            int dstOff = i * 2;
            if (bigEndian)
            {
                result[dstOff] = (byte)(pixel >> 8);
                result[dstOff + 1] = (byte)pixel;
            }
            else
            {
                result[dstOff] = (byte)pixel;
                result[dstOff + 1] = (byte)(pixel >> 8);
            }
        }
        return result;
    }

    // ---- RGB555 encoder ----
    internal static byte[] EncodeRgb555(ReadOnlySpan<byte> bgra, int w, int h, bool bigEndian)
    {
        int pixelCount = w * h;
        var result = new byte[pixelCount * 2];
        for (int i = 0; i < pixelCount; i++)
        {
            int srcOff = i * 4;
            int r = bgra[srcOff + 2] >> 3;  // R: top 5 bits
            int g = bgra[srcOff + 1] >> 3;  // G: top 5 bits
            int b = bgra[srcOff] >> 3;      // B: top 5 bits
            ushort pixel = (ushort)(r << 10 | g << 5 | b); // bit 15 unused

            int dstOff = i * 2;
            if (bigEndian)
            {
                result[dstOff] = (byte)(pixel >> 8);
                result[dstOff + 1] = (byte)pixel;
            }
            else
            {
                result[dstOff] = (byte)pixel;
                result[dstOff + 1] = (byte)(pixel >> 8);
            }
        }
        return result;
    }

    // ---- UYVY encoder (BT.601) ----
    internal static byte[] EncodeUyvy(ReadOnlySpan<byte> bgra, int w, int h)
    {
        // UYVY: each 4-byte block = [U, Y0, V, Y1] for 2 pixels
        var result = new byte[w * h * 2];
        for (int y = 0; y < h; y++)
        {
            for (int x = 0; x < w; x += 2)
            {
                int px0 = (y * w + x) * 4;
                int px1 = (y * w + x + 1) * 4;

                // Two pixels, one shared U/V
                int r0 = bgra[px0 + 2], g0 = bgra[px0 + 1], b0 = bgra[px0];
                int r1 = bgra[px1 + 2], g1 = bgra[px1 + 1], b1 = bgra[px1];

                int y0 = Bt601Y(r0, g0, b0);
                int y1 = Bt601Y(r1, g1, b1);
                // Average chroma for the pair
                int u = (Bt601Cb(r0, g0, b0) + Bt601Cb(r1, g1, b1) + 1) >> 1;
                int v = (Bt601Cr(r0, g0, b0) + Bt601Cr(r1, g1, b1) + 1) >> 1;

                int dstOff = (y * w + x) * 2;
                result[dstOff] = ClampU8(u);
                result[dstOff + 1] = ClampU8(y0);
                result[dstOff + 2] = ClampU8(v);
                result[dstOff + 3] = ClampU8(y1);
            }
        }
        return result;
    }

    // ---- YCbCr 4:2:0 encoder (BT.601, planar) ----
    internal static byte[] EncodeYcbcr420(ReadOnlySpan<byte> bgra, int w, int h,
        bool swapChromaPlanes = false)
    {
        int ySize = w * h;
        int uvW = (w + 1) / 2;
        int uvH = (h + 1) / 2;
        int uvSize = uvW * uvH;
        var result = new byte[ySize + uvSize * 2];

        // Y plane (full resolution)
        for (int i = 0; i < ySize; i++)
        {
            int off = i * 4;
            result[i] = ClampU8(Bt601Y(bgra[off + 2], bgra[off + 1], bgra[off]));
        }

        // Cb, Cr planes (2×2 averaged chroma)
        // When swapChromaPlanes is true, exchange storage order so the decoder
        // reading Cr from uvIdx and Cb from uvIdx+uvSize gets the correct values.
        for (int y = 0; y < h; y += 2)
        {
            for (int x = 0; x < w; x += 2)
            {
                int sumCb = 0, sumCr = 0, count = 0;
                for (int dy = 0; dy < 2 && y + dy < h; dy++)
                {
                    for (int dx = 0; dx < 2 && x + dx < w; dx++)
                    {
                        int px = ((y + dy) * w + (x + dx)) * 4;
                        sumCb += Bt601Cb(bgra[px + 2], bgra[px + 1], bgra[px]);
                        sumCr += Bt601Cr(bgra[px + 2], bgra[px + 1], bgra[px]);
                        count++;
                    }
                }
                int uvIdx = ySize + (y / 2) * uvW + (x / 2);
                byte cbVal = ClampU8((sumCb + count / 2) / count);
                byte crVal = ClampU8((sumCr + count / 2) / count);
                if (swapChromaPlanes)
                {
                    result[uvIdx] = crVal;
                    result[uvIdx + uvSize] = cbVal;
                }
                else
                {
                    result[uvIdx] = cbVal;
                    result[uvIdx + uvSize] = crVal;
                }
            }
        }
        return result;
    }

    // ---- CLCL nibble-chroma encoder ----
    internal static byte[] EncodeClcl(ReadOnlySpan<byte> bgra, int w, int h)
    {
        // CLCL: 2-bytes-per-pixel, chroma packed as 4-bit nibbles
        // Byte layout per macropixel (2 pixels): [CbCr_nibbles] [Y0] [CbCr_nibbles] [Y1] — 4 bytes
        // See Decoding.cs DecodeYuv422Clcl for the inverse
        // Requires even width (matching decoder's (w & 1) guard).
        int pixelCount = w * h;
        if ((w & 1) != 0) return Array.Empty<byte>();
        var result = new byte[pixelCount * 2];

        for (int i = 0; i < pixelCount; i += 2)
        {
            int px0 = i * 4, px1 = (i + 1) * 4;

            int r0 = bgra[px0 + 2], g0 = bgra[px0 + 1], b0 = bgra[px0];
            int r1 = bgra[px1 + 2], g1 = bgra[px1 + 1], b1 = bgra[px1];

            int y0 = Bt601Y(r0, g0, b0);
            int y1 = Bt601Y(r1, g1, b1);

            // Average chroma across the pair (same as UYVY approach)
            int cb = (Bt601Cb(r0, g0, b0) + Bt601Cb(r1, g1, b1) + 1) >> 1;
            int cr = (Bt601Cr(r0, g0, b0) + Bt601Cr(r1, g1, b1) + 1) >> 1;

            // Pack chroma as 4-bit nibbles: Cb in high 4 bits, Cr in low 4 bits
            int cbNibble = ClampU8(cb) >> 4;
            int crNibble = ClampU8(cr) >> 4;
            byte chromaNibble = (byte)((cbNibble << 4) | crNibble);

            int dstOff = i * 2;
            result[dstOff]     = chromaNibble;          // [0]: packed CbCr
            result[dstOff + 1] = ClampU8(y0);           // [1]: Y0
            result[dstOff + 2] = chromaNibble;          // [2]: packed CbCr (same)
            result[dstOff + 3] = ClampU8(y1);           // [3]: Y1
        }
        return result;
    }

    // ---- CL per-pixel nibble-chroma encoder (Keith's CL, Methods 3/4) ----
    internal static byte[] EncodeCl(ReadOnlySpan<byte> bgra, int w, int h)
    {
        // CL: 2-bytes-per-pixel, each pixel has its own chroma nibble
        // Byte layout per pixel: [Cb:Cr_nibble][Y] — 2 bytes, 1 pixel
        int pixelCount = w * h;
        var result = new byte[pixelCount * 2];

        for (int i = 0; i < pixelCount; i++)
        {
            int px = i * 4;
            int r = bgra[px + 2], g = bgra[px + 1], b = bgra[px];

            int y = Bt601Y(r, g, b);
            int cb = Bt601Cb(r, g, b);
            int cr = Bt601Cr(r, g, b);

            // Pack chroma as 4-bit nibbles: Cb in high 4 bits, Cr in low 4 bits
            int cbNibble = ClampU8(cb) >> 4;
            int crNibble = ClampU8(cr) >> 4;
            byte chromaNibble = (byte)((cbNibble << 4) | crNibble);

            int dstOff = i * 2;
            result[dstOff]     = chromaNibble;  // [0]: packed CbCr
            result[dstOff + 1] = ClampU8(y);    // [1]: Y
        }
        return result;
    }

    // ---- File builder: creates a complete F-prefix .ithmb file ----
    internal static byte[] BuildIthmbFile(ReadOnlySpan<byte> bgra, int w, int h, IthmbVariantProfile profile)
    {
        int fw = w, fh = h;
        if (profile.SwapsDimensions) { fw = h; fh = w; }

        // 1. Apply pre-encode rotation (undecode the decode rotation)
        ReadOnlySpan<byte> src = bgra;
        byte[]? rotatedBuf = null;
        if (profile.Rotation != 0 && w > 0 && h > 0)
        {
            rotatedBuf = new byte[w * h * 4];
            bgra.CopyTo(rotatedBuf);
            unsafe
            {
                fixed (byte* p = rotatedBuf)
                {
                    int rw = w, rh = h;
                    // Reverse rotation: decode rotation + encode = identity
                    int revRotation = (360 - profile.Rotation) % 360;
                    RotateBgra(p, ref rw, ref rh, revRotation);
                    fw = rw; fh = rh; // Update dimensions after rotation
                }
            }
            src = rotatedBuf;
        }

        // 2. Encode pixels. CL/CLCL chroma packing replaces the initial encode
        //    (they re-encode from original BGRA with different chroma precision).
        //    Interlacing is applied AFTER chroma packing so field order is correct.
        byte[] encoded;
        if (profile.ClChroma)
        {
            encoded = EncodeCl(src, fw, fh);
        }
        else if (profile.ClclChroma)
        {
            encoded = EncodeClcl(src, fw, fh);
        }
        else
        {
            encoded = profile.Encoding switch
            {
                IthmbEncoding.Rgb565 => EncodeRgb565(src, fw, fh, !profile.LittleEndian),
                IthmbEncoding.Rgb555 => EncodeRgb555(src, fw, fh, !profile.LittleEndian),
                IthmbEncoding.Yuv422 => EncodeUyvy(src, fw, fh),
                IthmbEncoding.Ycbcr420 => EncodeYcbcr420(src, fw, fh, profile.SwapChromaPlanes),
                _ => throw new ArgumentException($"Unknown encoding: {profile.Encoding}")
            };
        }

        // 3. Handle interlaced reordering (F1019: even fields first, then odd fields)
        //    Applied after chroma packing — field data is the final pixel format.
        if (profile.IsInterlaced)
        {
            encoded = InterlaceFields(encoded, fw, fh, profile.Encoding);
        }

        // 5. Pad to FrameByteLength if needed
        
if (profile.IsPadded && encoded.Length < profile.FrameByteLength)
        {
            var padded = new byte[profile.FrameByteLength];
            encoded.CopyTo(padded, 0);
            encoded = padded;
        }

        // 6. Prepend 4-byte big-endian prefix
        var result = new byte[4 + encoded.Length];
        result[0] = (byte)(profile.Prefix >> 24);
        result[1] = (byte)(profile.Prefix >> 16);
        result[2] = (byte)(profile.Prefix >> 8);
        result[3] = (byte)profile.Prefix;
        encoded.CopyTo(result, 4);

        return result;
    }

    // ---- Helper: interleave fields for interlaced formats ----
    private static byte[] InterlaceFields(byte[] planar, int w, int h, IthmbEncoding enc)
    {
        int bpp = enc == IthmbEncoding.Ycbcr420 ? 1 : 2; // YCbCr420 is planar (1 Bpp luma), others are 2 Bpp
        int rowStride = w * bpp;
        int halfRows = (h + 1) / 2;
        var result = new byte[planar.Length];

        // Even fields first, then odd fields
        for (int y = 0; y < h; y++)
        {
            int srcRow = y * rowStride;
            int dstRow = y % 2 == 0
                ? (y / 2) * rowStride
                : (halfRows + y / 2) * rowStride;

            Array.Copy(planar, srcRow, result, dstRow, rowStride);
        }
        return result;
    }

    // ---- BT.601 forward transform helpers (fixed-point, same precision as decoder) ----
    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static int Bt601Y(int r, int g, int b)
    {
        // Y = 0.299*R + 0.587*G + 0.114*B  (fixed-point: 77/256, 150/256, 29/256)
        return (77 * r + 150 * g + 29 * b) >> 8;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static int Bt601Cb(int r, int g, int b)
    {
        // Cb = -0.169*R - 0.331*G + 0.500*B + 128
        // Use >> 8 (arithmetic shift) to match decoder's rounding direction
        return ((-43 * r - 85 * g + 128 * b) >> 8) + 128;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static int Bt601Cr(int r, int g, int b)
    {
        // Cr = 0.500*R - 0.419*G - 0.081*B + 128
        // Use >> 8 to match decoder's rounding
        return ((128 * r - 107 * g - 21 * b) >> 8) + 128;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static byte ClampU8(int v) => (byte)(v < 0 ? 0 : v > 255 ? 255 : v);
}
