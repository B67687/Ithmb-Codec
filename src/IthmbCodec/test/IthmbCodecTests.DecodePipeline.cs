using Xunit;
using ImageGlass.SDK.Plugins;
using System.Runtime.InteropServices;

namespace IthmbCodec.Tests;

public unsafe class DecodePipelineTests
{
    /// <summary>
    /// When DecodeInternal receives a file with an unknown prefix and no embedded JPEG,
    /// it should fall through all detection paths and return DecodeFailed.
    /// Exercises the no-profile-no-JPEG tail of the decode pipeline (lines 252-253).
    /// </summary>
    [Fact]
    public void UnknownPrefix_NoJpeg_ReturnsDecodeFailed()
    {
        byte[] content =
        [
            0xDE, 0xAD, 0xBE, 0xEF,
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
            0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19,
            0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29
        ];

        string tmpPath = Path.Combine(Path.GetTempPath(), $"ithmb_unknown_nojpeg_{Guid.NewGuid():N}.ithmb");
        try
        {
            File.WriteAllBytes(tmpPath, content);
            var outInfo = (IGImageInfo*)NativeMemory.AllocZeroed((nuint)sizeof(IGImageInfo));
            var outBuf = (IGPixelBuffer*)NativeMemory.AllocZeroed((nuint)sizeof(IGPixelBuffer));
            try
            {
                char[] pathChars = (tmpPath + "\0").ToCharArray();
                fixed (char* pPath = pathChars)
                {
                    var filePath = new IGStringRef { Data = pPath, Length = pathChars.Length - 1 };
                    var status = IthmbCodecPlugin.DecodeInternal(filePath, cancellation: null, outInfo, outBuf);
                    Assert.Equal(IGStatus.DecodeFailed, status);
                }
            }
            finally
            {
                if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
                NativeMemory.Free(outInfo);
                NativeMemory.Free(outBuf);
            }
        }
        finally
        {
            if (File.Exists(tmpPath)) File.Delete(tmpPath);
        }
    }

    /// <summary>
    /// When DecodeInternal receives a file with an unknown prefix but an embedded JPEG
    /// found by carving, it should decode the JPEG successfully.
    /// Exercises the full carving-to-BGRA pipeline including DecodeJpegSlice (lines 243-249).
    /// </summary>
    [Fact]
    public void UnknownPrefix_EmbeddedJpeg_DecodesSuccessfully()
    {
        byte[] jpeg = BuildMinimalJpeg(Jfif: true);
        var content = new byte[4 + 256 + jpeg.Length];
        content[0] = 0xDE; content[1] = 0xAD; content[2] = 0xBE; content[3] = 0xEF;
        Array.Copy(jpeg, 0, content, 260, jpeg.Length);

        string tmpPath = Path.Combine(Path.GetTempPath(), $"ithmb_carving_jpeg_{Guid.NewGuid():N}.ithmb");
        try
        {
            File.WriteAllBytes(tmpPath, content);
            var outInfo = (IGImageInfo*)NativeMemory.AllocZeroed((nuint)sizeof(IGImageInfo));
            var outBuf = (IGPixelBuffer*)NativeMemory.AllocZeroed((nuint)sizeof(IGPixelBuffer));
            try
            {
                char[] pathChars = (tmpPath + "\0").ToCharArray();
                fixed (char* pPath = pathChars)
                {
                    var filePath = new IGStringRef { Data = pPath, Length = pathChars.Length - 1 };
                    var status = IthmbCodecPlugin.DecodeInternal(filePath, cancellation: null, outInfo, outBuf);
                    Assert.Equal(IGStatus.OK, status);
                    Assert.Equal(1, outBuf->Width);
                    Assert.Equal(1, outBuf->Height);
                }
            }
            finally
            {
                if (outBuf->Data != null) NativeMemory.Free((void*)outBuf->Data);
                NativeMemory.Free(outInfo);
                NativeMemory.Free(outBuf);
            }
        }
        finally
        {
            if (File.Exists(tmpPath)) File.Delete(tmpPath);
        }
    }

    private static byte[] BuildMinimalJpeg(bool Jfif = false, bool Exif = false)
    {
        using var ms = new MemoryStream();
        ms.Write([0xFF, 0xD8]);
        if (Jfif)
        {
            ms.Write([0xFF, 0xE0]);
            ms.WriteByte(0x00); ms.WriteByte(0x10);
            ms.Write([(byte)'J', (byte)'F', (byte)'I', (byte)'F', 0]);
            ms.WriteByte(0x01); ms.WriteByte(0x01);
            ms.WriteByte(0x00);
            ms.WriteByte(0x00); ms.WriteByte(0x01);
            ms.WriteByte(0x00); ms.WriteByte(0x01);
            ms.WriteByte(0x00); ms.WriteByte(0x00);
        }
        if (Exif)
        {
            ms.Write([0xFF, 0xE1]);
            ms.WriteByte(0x00); ms.WriteByte(0x27);
            ms.Write([(byte)'E', (byte)'x', (byte)'i', (byte)'f', 0, 0]);
            ms.Write([(byte)'I', (byte)'I', 0x2A, 0x00]);
            ms.WriteByte(0x08); ms.WriteByte(0x00); ms.WriteByte(0x00); ms.WriteByte(0x00);
            ms.WriteByte(0x00); ms.WriteByte(0x00);
            ms.Write([0x00, 0x00, 0x00, 0x00]);
        }
        ms.Write([0xFF, 0xDB, 0x00, 0x43, 0]);
        for (int i = 0; i < 64; i++) ms.WriteByte(1);
        ms.Write([0xFF, 0xC0, 0x00, 0x0B, 0x08]);
        ms.WriteByte(0x00); ms.WriteByte(0x01);
        ms.WriteByte(0x00); ms.WriteByte(0x01);
        ms.WriteByte(0x01);
        ms.WriteByte(0x01); ms.WriteByte(0x11); ms.WriteByte(0x00);
        ms.Write([0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00]);
        ms.WriteByte(0x80);
        ms.Write([0xFF, 0xD9]);
        return ms.ToArray();
    }
}
