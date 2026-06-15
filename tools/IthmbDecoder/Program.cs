// CLI tool that decodes .ithmb files using the plugin's actual decode pipeline.
// Uses the exact same code as the ImageGlass plugin — no separate extraction logic.
//
// Usage:
//   dotnet run --project tools/IthmbDecoder -- <input.ithmb> [output.bmp]
//
// If output path is omitted, writes to <input>.bmp in the same directory.

using System.Runtime.InteropServices;
using IthmbCodec;
using ImageGlass.SDK.Plugins;

namespace IthmbDecoder;

unsafe class Program
{
    static int Main(string[] args)
    {
        if (args.Length < 1 || args[0] == "--help" || args[0] == "-h")
        {
            Console.Error.WriteLine("Usage: IthmbDecoder <input.ithmb> [output.bmp]");
            Console.Error.WriteLine("Decodes an Apple .ithmb thumbnail-cache file to BMP.");
            Console.Error.WriteLine("If output path is omitted, writes to <input>.bmp in the same directory.");
            return 0;
        }

        string inputPath = args[0];
        if (!File.Exists(inputPath))
        {
            Console.Error.WriteLine($"File not found: {inputPath}");
            return 1;
        }

        string outputPath = args.Length > 1 ? args[1] : Path.ChangeExtension(inputPath, ".bmp");
        // Resolve to full path to prevent path traversal
        outputPath = Path.GetFullPath(outputPath);

        // Build an IGStringRef for the file path (UTF-16, as expected by the plugin)
        char[] pathChars = (inputPath + "\0").ToCharArray();
        fixed (char* pPath = pathChars)
        {
            var filePath = new IGStringRef { Data = pPath, Length = pathChars.Length - 1 };
            var outInfo = (IGImageInfo*)NativeMemory.AllocZeroed((nuint)sizeof(IGImageInfo));
            var outBuf = (IGPixelBuffer*)NativeMemory.AllocZeroed((nuint)sizeof(IGPixelBuffer));
            if (outInfo == null || outBuf == null)
            {
                Console.Error.WriteLine("Out of memory");
                return 1;
            }

            try
            {
                var status = IthmbCodecPlugin.DecodeInternal(filePath, cancellation: null, outInfo, outBuf);

                if (status != IGStatus.OK)
                {
                    Console.Error.WriteLine($"Decode failed: {status} ({(int)status})");
                    return (int)status;
                }

                if (outBuf->Data == null)
                {
                    Console.Error.WriteLine("Decode returned OK but buffer is null");
                    return 1;
                }

                int w = outInfo->Width;
                int h = outInfo->Height;
                Console.Error.WriteLine($"Decoded: {w}x{h}, orientation={outInfo->Orientation}");

                // Convert BGRA to RGBA for StbImageSharp
                int pixelCount = w * h;
                var rgba = new byte[pixelCount * 4];
                var src = new Span<byte>((void*)outBuf->Data, pixelCount * 4);
                for (int i = 0; i < pixelCount; i++)
                {
                    int off = i * 4;
                    rgba[off]     = src[off + 2];  // R
                    rgba[off + 1] = src[off + 1];  // G
                    rgba[off + 2] = src[off];      // B
                    rgba[off + 3] = 255;           // A
                }

                // Write as BMP (no external dependencies, natively supported by Windows/ImageGlass)
                WriteRgbaAsBmp(rgba, w, h, outputPath);
                Console.Error.WriteLine($"Written: {outputPath}");
            }
            finally
            {
                NativeMemory.Free(outInfo);
                IthmbCodecPlugin.FreePixelBuffer(outBuf);
                NativeMemory.Free(outBuf);
            }
        }

        return 0;
    }

    static void WriteRgbaAsBmp(byte[] rgba, int w, int h, string path)
    {
        // BMP format: 14-byte header + 40-byte DIB header + pixel data (BGRA, bottom-up)
        // Input is RGBA; remap to BGRA for BMP.
        int rowStride = ((w * 32 + 31) / 32) * 4; // BMP row alignment to 4 bytes
        int pixelDataSize = rowStride * h;
        int fileSize = 14 + 40 + pixelDataSize;

        using var fs = new FileStream(path, FileMode.Create);
        using var bw = new BinaryWriter(fs);

        // BITMAPFILEHEADER (14 bytes)
        bw.Write((byte)'B'); bw.Write((byte)'M');     // signature
        bw.Write(fileSize);                             // file size
        bw.Write((ushort)0);                            // reserved1
        bw.Write((ushort)0);                            // reserved2
        bw.Write(14 + 40);                              // offset to pixel data

        // BITMAPINFOHEADER (40 bytes)
        bw.Write(40);                                   // header size
        bw.Write(w);                                    // width
        bw.Write(h);                                    // height (positive = bottom-up)
        bw.Write((ushort)1);                            // planes
        bw.Write((ushort)32);                           // bits per pixel
        bw.Write(0);                                    // compression (none)
        bw.Write(pixelDataSize);                        // image size
        bw.Write(0);                                    // x pixels per meter
        bw.Write(0);                                    // y pixels per meter
        bw.Write(0);                                    // colors used
        bw.Write(0);                                    // important colors

        // Pixel data (bottom-up: write last row first)
        byte[] row = new byte[rowStride];
        for (int y = h - 1; y >= 0; y--)
        {
            int srcOff = y * w * 4;
            for (int x = 0; x < w; x++)
            {
                int off = srcOff + x * 4;
                row[x * 4]     = rgba[off + 2];  // B (from RGBA)
                row[x * 4 + 1] = rgba[off + 1];  // G
                row[x * 4 + 2] = rgba[off];      // R (from RGBA)
                row[x * 4 + 3] = 255;            // A (opaque)
            }
            bw.Write(row, 0, rowStride);
        }
    }
}
