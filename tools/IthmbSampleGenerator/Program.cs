// Generates a synthetic .ithmb sample file with a geometric test pattern.
// This is 100% our own IP — no personal data, no Apple copyrighted content.
// Use this file to take a screenshot of ImageGlass displaying decoded .ithmb output.
//
// Usage:
//   dotnet run --project tools/IthmbSampleGenerator
//   Output: sample.ithmb (for ImageGlass) and sample_original.bmp (reference)

using IthmbCodec;
using System.Runtime.InteropServices;

namespace IthmbSampleGenerator;

unsafe class Program
{
    static void Main()
    {
        int w = 320, h = 240;
        byte[] bgra = CreateTestPattern(w, h);

        // Use RGB565 encoding (profile 1024 = iPod Classic 5G/6G full-screen).
        // BuildIthmbFile prepends the 4-byte prefix, so the file is auto-detected
        // as profile 1024 and decoded at the correct resolution.
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1024, Width: w, Height: h,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: w * h * 2);

        byte[] ithmbFile = IthmbCodecPlugin.BuildIthmbFile(bgra, w, h, profile);
        File.WriteAllBytes("sample.ithmb", ithmbFile);
        Console.WriteLine($"Created sample.ithmb ({ithmbFile.Length} bytes)");

        // Also save the original as BMP for reference
        WriteBgraAsBmp(bgra, w, h, "sample_original.bmp");
        Console.WriteLine("Created sample_original.bmp");
    }

    /// <summary>Creates a 640x480 BGRA test pattern with gradient + geometric design.</summary>
    static byte[] CreateTestPattern(int w, int h)
    {
        byte[] pixels = new byte[w * h * 4];
        for (int y = 0; y < h; y++)
        {
            for (int x = 0; x < w; x++)
            {
                int off = (y * w + x) * 4;
                float nx = (float)x / w;   // 0..1
                float ny = (float)y / h;   // 0..1

                // Background: dark gradient (navy to purple)
                byte bgR = (byte)(20 + ny * 50);
                byte bgG = (byte)(15 + ny * 30);
                byte bgB = (byte)(40 + ny * 60);

                // Four encoding-type color bands across top
                if (y < 30)
                {
                    int band = x / (w / 4);
                    switch (band)
                    {
                        case 0: bgR = 233; bgG = 69; bgB = 96; break;   // RGB565 — Red
                        case 1: bgR = 15; bgG = 52; bgB = 96; break;    // RGB555 — Blue
                        case 2: bgR = 83; bgG = 52; bgB = 131; break;   // UYVY — Purple
                        case 3: bgR = 0; bgG = 150; bgB = 136; break;   // YCbCr420 — Teal
                    }
                }
                // Logo area: "ithmb-codec" rendered as pixel pattern
                else if (y < 50)
                {
                    // Geometric logo: 4 small squares and a line
                    int sqSize = 6;
                    for (int i = 0; i < 4; i++)
                    {
                        int sx = 20 + i * (sqSize + 4);
                        int sy = 34;
                        if (x >= sx && x < sx + sqSize && y >= sy && y < sy + sqSize)
                        {
                            bgR = (byte)(233 - i * 50);
                            bgG = (byte)(69 + i * 25);
                            bgB = (byte)(96 + i * 30);
                        }
                    }
                }
                // Central color wheel — main showcase
                else if (y > 55 && y < 195 && x > 40 && x < 280)
                {
                    float cx = (x - 160f) / 120f;
                    float cy = (y - 125f) / 70f;
                    float dist = MathF.Sqrt(cx * cx + cy * cy);
                    float angle = MathF.Atan2(cy, cx);

                    if (dist < 1.0f)
                    {
                        float hue = (angle / MathF.PI + 1) / 2;
                        HsvToRgb(hue, dist, 0.9f, out bgR, out bgG, out bgB);
                    }
                    // Subtle checkerboard at edges
                    if (dist > 0.8f && ((x / 8 + y / 8) % 2 == 0))
                    {
                        bgR = (byte)(bgR * 0.7f);
                        bgG = (byte)(bgG * 0.7f);
                        bgB = (byte)(bgB * 0.7f);
                    }
                }
                // Bottom: color bars + grid
                else if (y > 200)
                {
                    int bar = (x * 8) / w;
                    bgR = (byte)((bar & 4) * 64);
                    bgG = (byte)((bar & 2) * 80);
                    bgB = (byte)((bar & 1) * 140);
                    if (x % 20 == 0 || y % 20 == 0)
                    { bgR = 255; bgG = 255; bgB = 255; }
                }

                pixels[off] = bgB;     // B
                pixels[off + 1] = bgG; // G
                pixels[off + 2] = bgR; // R
                pixels[off + 3] = 255; // A
            }
        }
        return pixels;
    }

    /// <summary>Approximates small text using a 5-pixel-wide bitmap font.</summary>
    static void DrawSmallText(byte[] pixels, int lx, int ly, string text,
        ref byte r, ref byte g, ref byte b)
    {
        // Simple text approximation: just darken the pixel so the text is readable
        if (lx < 5 * text.Length && ly >= 0 && ly < 12)
            r = 0; g = 0; b = 0;
    }

    /// <summary>HSV → RGB conversion (floating point, all 0..1).</summary>
    static void HsvToRgb(float h, float s, float v, out byte r, out byte g, out byte b)
    {
        if (s == 0) { r = (byte)(v * 255); g = r; b = r; return; }
        h = (h % 1 + 1) % 1;
        int i = (int)(h * 6);
        float f = h * 6 - i;
        float p = v * (1 - s);
        float q = v * (1 - s * f);
        float t = v * (1 - s * (1 - f));
        float rF, gF, bF;
        switch (i % 6)
        {
            case 0: rF = v; gF = t; bF = p; break;
            case 1: rF = q; gF = v; bF = p; break;
            case 2: rF = p; gF = v; bF = t; break;
            case 3: rF = p; gF = q; bF = v; break;
            case 4: rF = t; gF = p; bF = v; break;
            default: rF = v; gF = p; bF = q; break;
        }
        r = (byte)(rF * 255); g = (byte)(gF * 255); b = (byte)(bF * 255);
    }

    /// <summary>Writes BGRA pixel data as a BMP file (bottom-up, 32-bit).</summary>
    static void WriteBgraAsBmp(byte[] bgra, int w, int h, string path)
    {
        int rowStride = ((w * 32 + 31) / 32) * 4;
        int pixelDataSize = rowStride * h;
        int fileSize = 14 + 40 + pixelDataSize;

        using var fs = new FileStream(path, FileMode.Create);
        using var bw = new BinaryWriter(fs);

        // BITMAPFILEHEADER
        bw.Write((byte)'B'); bw.Write((byte)'M');
        bw.Write(fileSize);
        bw.Write((ushort)0);
        bw.Write((ushort)0);
        bw.Write(14 + 40);

        // BITMAPINFOHEADER
        bw.Write(40);
        bw.Write(w);
        bw.Write(h);
        bw.Write((ushort)1);
        bw.Write((ushort)32);
        bw.Write(0);
        bw.Write(pixelDataSize);
        bw.Write(0); bw.Write(0);
        bw.Write(0); bw.Write(0);

        // Pixel data (bottom-up: write last row first, BGRA order)
        byte[] row = new byte[rowStride];
        for (int y = h - 1; y >= 0; y--)
        {
            int srcOff = y * w * 4;
            for (int x = 0; x < w; x++)
            {
                int off = srcOff + x * 4;
                row[x * 4] = bgra[off];      // B
                row[x * 4 + 1] = bgra[off + 1]; // G
                row[x * 4 + 2] = bgra[off + 2]; // R
                row[x * 4 + 3] = 255;         // A
            }
            bw.Write(row, 0, rowStride);
        }
    }
}
