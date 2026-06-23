// CLI tool that decodes .ithmb files using the plugin's actual decode pipeline.
// Uses the exact same code as the ImageGlass plugin — no separate extraction logic.
//
// Usage:
//   dotnet run --project tools/IthmbDecoder -- <input.ithmb> [output.bmp]
//   dotnet run --project tools/IthmbDecoder -- --list-pd <PhotoDB|ArtworkDB>
//   dotnet run --project tools/IthmbDecoder -- --pd-index <N> <PhotoDB> [output.bmp]
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
        bool listPd = false;
        int? pdIndex = null;
        string? inputPath = null;
        string? outputPath = null;

        for (int i = 0; i < args.Length; i++)
        {
            if (args[i] == "--list-pd")
                listPd = true;
            else if (args[i] == "--pd-index" && i + 1 < args.Length)
                pdIndex = int.Parse(args[++i]);
            else if (args[i] == "--help" || args[i] == "-h")
            {
                PrintUsage();
                return 0;
            }
            else if (inputPath == null)
                inputPath = args[i];
            else if (outputPath == null)
                outputPath = args[i];
        }

        if (inputPath == null)
        {
            PrintUsage();
            return 1;
        }

        if (!File.Exists(inputPath))
        {
            Console.Error.WriteLine($"File not found: {inputPath}");
            return 1;
        }

        if (listPd)
            return ListPhotoDbEntries(inputPath);

        if (pdIndex.HasValue)
            return DecodePhotoDbEntry(inputPath, pdIndex.Value, outputPath);

        outputPath ??= Path.ChangeExtension(inputPath, ".bmp");
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

    static void PrintUsage()
    {
        Console.Error.WriteLine("Usage:");
        Console.Error.WriteLine("  IthmbDecoder <input.ithmb> [output.bmp]    Decode .ithmb file to BMP");
        Console.Error.WriteLine("  IthmbDecoder --list-pd <PhotoDB|ArtworkDB>  List PhotoDB/ArtworkDB entries");
        Console.Error.WriteLine("  IthmbDecoder --pd-index <N> <PhotoDB> [output.bmp]  Extract entry N from PhotoDB");
        Console.Error.WriteLine();
        Console.Error.WriteLine("PhotoDB/ArtworkDB are Apple's iPod/iPhone thumbnail database files");
        Console.Error.WriteLine("(typically named \"Photo Database\" or \"Artwork Database\" with no extension).");
    }

    /// <summary>Lists all entries in a PhotoDB/ArtworkDB file.</summary>
    static int ListPhotoDbEntries(string path)
    {
        byte[] data = File.ReadAllBytes(path);
        if (!IthmbCodecPlugin.TryParsePhotoDb(data, out var entries, out var frameCount))
        {
            Console.Error.WriteLine("Not a valid PhotoDB/ArtworkDB file (no MHFD magic found).");
            return 1;
        }

        Console.Error.WriteLine($"PhotoDB entries: {frameCount}");
        Console.Error.WriteLine($"{"Idx",4}  {"Format",8}  {"Width",6}  {"Height",6}  {"Size (B)",8}  {"Description"}");
        Console.Error.WriteLine(new string('-', 70));

        for (int i = 0; i < entries.Count; i++)
        {
            var (fmtId, entryData) = entries[i];
            string fmtName = IthmbCodecPlugin.GetFormatIdName(fmtId);
            if (IthmbCodecPlugin.KnownProfiles.TryGetValue(fmtId, out var profile))
            {
                Console.Error.WriteLine($"{i,4}  {fmtId,8}  {profile.Width,6}  {profile.Height,6}  {entryData.Length,8}  {profile.Encoding} (LE={profile.LittleEndian})");
            }
            else
            {
                Console.Error.WriteLine($"{i,4}  {fmtId,8}  {"?",6}  {"?",6}  {entryData.Length,8}  Unknown format");
            }
        }

        return 0;
    }

    /// <summary>Decodes a single PhotoDB entry by index and writes it as BMP.</summary>
    static int DecodePhotoDbEntry(string path, int index, string? outputPath)
    {
        byte[] data = File.ReadAllBytes(path);
        if (!IthmbCodecPlugin.TryParsePhotoDb(data, out var entries, out var frameCount))
        {
            Console.Error.WriteLine("Not a valid PhotoDB/ArtworkDB file.");
            return 1;
        }

        if (index < 0 || index >= entries.Count)
        {
            Console.Error.WriteLine($"Entry index {index} out of range (0-{entries.Count - 1}).");
            return 1;
        }

        var (fmtId, rawData) = entries[index];

        if (!IthmbCodecPlugin.KnownProfiles.TryGetValue(fmtId, out var profile))
        {
            Console.Error.WriteLine($"Format {fmtId} is not a known profile. Cannot decode.");
            return 1;
        }

        int w = profile.Width, h = profile.Height;
        if (w <= 0 || h <= 0)
        {
            Console.Error.WriteLine($"Invalid dimensions: {w}x{h}");
            return 1;
        }

        outputPath ??= Path.ChangeExtension(path, $".entry{index}.bmp");
        outputPath = Path.GetFullPath(outputPath);

        byte* pixels = (byte*)NativeMemory.AllocZeroed((nuint)(w * 4 * h));
        if (pixels == null)
        {
            Console.Error.WriteLine("Out of memory");
            return 1;
        }

        try
        {
            bool ok = profile.Encoding switch
            {
                IthmbCodecPlugin.IthmbEncoding.Rgb565 => IthmbCodecPlugin.DecodeRgb565(rawData, pixels, w, h, profile.LittleEndian),
                IthmbCodecPlugin.IthmbEncoding.Rgb555 => IthmbCodecPlugin.DecodeRgb555(rawData, pixels, w, h, profile.LittleEndian),
                IthmbCodecPlugin.IthmbEncoding.Yuv422 => profile.ClChroma
                    ? IthmbCodecPlugin.DecodeYuv422Cl(rawData, pixels, w, h)
                    : profile.ClclChroma
                    ? IthmbCodecPlugin.DecodeYuv422Clcl(rawData, pixels, w, h)
                    : profile.IsInterlaced
                    ? IthmbCodecPlugin.DecodeYuv422Interlaced(rawData, pixels, w, h)
                    : IthmbCodecPlugin.DecodeYuv422(rawData, pixels, w, h),
                IthmbCodecPlugin.IthmbEncoding.Ycbcr420 => IthmbCodecPlugin.DecodeYcbcr420(rawData, pixels, w, h, profile.SwapChromaPlanes),
                _ => false,
            };

            if (!ok)
            {
                Console.Error.WriteLine($"Decode failed for format {fmtId} ({profile.Encoding}).");
                return 1;
            }

            int pixelCount = w * h;
            var rgba = new byte[pixelCount * 4];
            for (int i = 0; i < pixelCount; i++)
            {
                int off = i * 4;
                rgba[off]     = pixels[off + 2];  // R
                rgba[off + 1] = pixels[off + 1];  // G
                rgba[off + 2] = pixels[off];      // B
                rgba[off + 3] = 255;              // A
            }

            WriteRgbaAsBmp(rgba, w, h, outputPath);
            Console.Error.WriteLine($"Entry {index}: format={fmtId} ({profile.Width}x{profile.Height}, {profile.Encoding}) -> {outputPath}");
        }
        finally
        {
            NativeMemory.Free(pixels);
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
