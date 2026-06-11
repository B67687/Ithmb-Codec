// Licensed under MIT. See LICENSE in the repository root.
// 
// Decode algorithms for .ithmb raw profiles.
// Separated from plugin ABI glue for independent AOT compilation.
using System;
using System.Runtime.CompilerServices;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // BT.601-7 (SDTV) Y′CbCr → R′G′B′ coefficients, 8:8 fixed-point
    private const int YuvRCoef = 359;   //  1.402  (Cr contribution to R)
    private const int YuvGCoefCb = 88;  // -0.344  (Cb contribution to G)
    private const int YuvGCoefCr = 183; // -0.714  (Cr contribution to G)
    private const int YuvBCoef = 454;   //  1.772  (Cb contribution to B)

    // ---------- RGB565 ----------

    internal static void DecodeRgb565(ReadOnlySpan<byte> src, byte* dst, int w, int h, bool littleEndian)
    {
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return;

        for (int y = 0; y < h; y++)
        {
            int srcRowBase = y * w * 2;
            byte* pDstRow = dst + y * w * 4;
            for (int x = 0; x < w; x++)
            {
                int idx = srcRowBase + x * 2;
                ushort rgb = littleEndian
                    ? (ushort)(src[idx] | (src[idx + 1] << 8))
                    : (ushort)((src[idx] << 8) | src[idx + 1]);
                int r5 = (rgb >> 11) & 0x1F;
                int g6 = (rgb >> 5) & 0x3F;
                int b5 = rgb & 0x1F;
                pDstRow[0] = (byte)((b5 << 3) | (b5 >> 2));   // B with MSB replication
                pDstRow[1] = (byte)((g6 << 2) | (g6 >> 4));   // G with MSB replication
                pDstRow[2] = (byte)((r5 << 3) | (r5 >> 2));   // R with MSB replication
                pDstRow[3] = 255;
                pDstRow += 4;
            }
        }
    }

    // ---------- YUV 4:2:2 (UYVY) ----------

    internal static void DecodeYuv422(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return;

        for (int y = 0; y < h; y++)
        {
            int rowStart = y * w * 2;
            byte* pDstRow = dst + y * w * 4;
            for (int x = 0; x < w; x += 2)
            {
                int idx = rowStart + x * 2;
                int u = src[idx] - 128;
                int y0 = src[idx + 1];
                int v = src[idx + 2] - 128;
                int y1 = src[idx + 3];

                WriteYuvPixel(pDstRow, y0, u, v);
                if (x + 1 < w) WriteYuvPixel(pDstRow + 4, y1, u, v);
                pDstRow += 8;
            }
        }
    }

    internal static void DecodeYuv422Interlaced(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        long expectedBytes = (long)w * h * 2;
        if (src.Length < expectedBytes) return;
        int half = (h / 2) * w * 2;
        int rowStride = w * 2;
        for (int y = 0; y < h; y++)
        {
            int fieldOffset = (y % 2 == 0) ? 0 : half;
            int rowInField = y / 2;
            int rowStart = fieldOffset + rowInField * rowStride;
            byte* pDstRow = dst + y * w * 4;
            for (int x = 0; x < w; x += 2)
            {
                int idx = rowStart + x * 2;
                int u = src[idx] - 128;
                int y0 = src[idx + 1];
                int v = src[idx + 2] - 128;
                int y1 = src[idx + 3];
                WriteYuvPixel(pDstRow, y0, u, v);
                if (x + 1 < w) WriteYuvPixel(pDstRow + 4, y1, u, v);
                pDstRow += 8;
            }
        }
    }

    // ---------- YCbCr 4:2:0 (planar) ----------

    internal static void DecodeYcbcr420(ReadOnlySpan<byte> src, byte* dst, int w, int h)
    {
        int ySize = w * h;
        int uvSize = ((w + 1) / 2) * ((h + 1) / 2);
        long expectedBytes = (long)ySize + uvSize * 2;
        if (src.Length < expectedBytes) return;

        int uvStride = w / 2;
        for (int y = 0; y < h; y += 2)
        {
            for (int x = 0; x < w; x += 2)
            {
                int uvIdx = ySize + (y / 2) * uvStride + (x / 2);
                int cb = src[uvIdx] - 128;
                int cr = src[uvIdx + uvSize] - 128;

                for (int dy = 0; dy < 2 && y + dy < h; dy++)
                {
                    int yRowStart = (y + dy) * w;
                    byte* pDstBlock = dst + (y + dy) * w * 4 + x * 4;
                    for (int dx = 0; dx < 2 && x + dx < w; dx++)
                    {
                        int yy = src[yRowStart + x + dx];
                        WriteYuvPixel(pDstBlock, yy, cb, cr);
                        pDstBlock += 4;
                    }
                }
            }
        }
    }

    // ---------- YUV→RGB conversion ----------

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static void WriteYuvPixel(byte* pDst, int luma, int cb, int cr)
    {
        int r = Clamp(luma + ((YuvRCoef * cr) >> 8));
        int g = Clamp(luma - ((YuvGCoefCb * cb) >> 8) - ((YuvGCoefCr * cr) >> 8));
        int b = Clamp(luma + ((YuvBCoef * cb) >> 8));
        pDst[0] = (byte)b; pDst[1] = (byte)g;
        pDst[2] = (byte)r; pDst[3] = 255;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static int Clamp(int v) => v < 0 ? 0 : v > 255 ? 255 : v;
}
