// SPDX-License-Identifier: MIT
// Encoder helper utilities: interlaced field encoding and BT.601 color conversion tables

using System;
using System.Runtime.CompilerServices;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // ---- Helper: interleave fields for interlaced formats ----
    private static byte[] InterlaceFields(byte[] planar, int w, int h, IthmbEncoding enc)
    {
        if (enc != IthmbEncoding.Ycbcr420)
        {
            // 2 Bpp interlace: each row is w * 2 bytes, just reorder rows
            int rowStride = w * 2;
            int halfRows = (h + 1) / 2;
            var result = new byte[planar.Length];
            for (int y = 0; y < h; y++)
            {
                int srcOff = y * rowStride;
                int dstOff = y % 2 == 0
                    ? (y / 2) * rowStride
                    : (halfRows + y / 2) * rowStride;
                Array.Copy(planar, srcOff, result, dstOff, rowStride);
            }
            return result;
        }

        // YCbCr 4:2:0 planar — 3 planes: Y (w*h), Cb (w/2*h/2), Cr (w/2*h/2)
        // Use ceiling division for chroma dimensions to match the encoder plane size
        int ySize = w * h;
        int uvW = (w + 1) / 2;
        int uvH = (h + 1) / 2;
        int cSize = uvW * uvH;
        int yRow = w;
        int cRow = uvW;
        int halfH = (h + 1) / 2;
        int halfH_uv = (uvH + 1) / 2;

        var result2 = new byte[planar.Length];

        // --- Interlace Y plane (full resolution) ---
        for (int y = 0; y < h; y++)
        {
            int srcOff = y * yRow;
            int dstOff = y % 2 == 0
                ? (y / 2) * yRow
                : (halfH + y / 2) * yRow;
            Array.Copy(planar, srcOff, result2, dstOff, yRow);
        }

        // --- Interlace Cb plane (half resolution) ---
        int cbOff = ySize;
        int cbDstBase = ySize;
        for (int y = 0; y < h / 2; y++)
        {
            int srcOff = cbOff + y * cRow;
            int dstOff = cbDstBase + (y % 2 == 0
                ? (y / 2) * cRow
                : (halfH_uv + y / 2) * cRow);
            Array.Copy(planar, srcOff, result2, dstOff, cRow);
        }

        // --- Interlace Cr plane (half resolution) ---
        int crOff = ySize + cSize;
        int crDstBase = ySize + cSize;
        for (int y = 0; y < h / 2; y++)
        {
            int srcOff = crOff + y * cRow;
            int dstOff = crDstBase + (y % 2 == 0
                ? (y / 2) * cRow
                : (halfH_uv + y / 2) * cRow);
            Array.Copy(planar, srcOff, result2, dstOff, cRow);
        }

        return result2;
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
        // Use >> 8 (arithmetic shift) to match decoder rounding direction
        return ((-43 * r - 85 * g + 128 * b) >> 8) + 128;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static int Bt601Cr(int r, int g, int b)
    {
        // Cr = 0.500*R - 0.419*G - 0.081*B + 128
        // Use >> 8 to match decoder rounding
        return ((128 * r - 107 * g - 21 * b) >> 8) + 128;
    }
}
