using System.Runtime.InteropServices;
using System.Security.Cryptography;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;




public unsafe partial class IthmbCodecTests
{
    private const string SyntheticVectorsDir = "../../../../../../samples/reuhno-synthetic/";

    /// <summary>
    /// Decodes frame 0 of F1061 (55x55 slot) and verifies the full BGRA SHA256,
    /// plus confirms magenta (0xF81F) padding outside the declared rect.
    /// </summary>
    [Fact]
    public void SyntheticVector_F1061_DecodeFrame0()
    {
        string path = Path.Combine(SyntheticVectorsDir, "F1061_1.ithmb");
        byte[] raw = File.ReadAllBytes(path);
        byte[] frame = raw[..6160];

        // F1061: 55x55 slot, RGB565 LE, stride 112, 6160 bytes per frame
        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1061, Width: 55, Height: 55,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: 6160);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(frame, profile,
                cancellation: null, outInfo, outBuf);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
            Assert.Equal(55, outBuf->Width);
            Assert.Equal(55, outBuf->Height);

            int bgraSize = 55 * 55 * 4;
            var decoded = new ReadOnlySpan<byte>((void*)outBuf->Data, bgraSize);

            // Verify full decoded output via SHA256
            string actualSha = Convert.ToHexStringLower(SHA256.HashData(decoded));
            Assert.Equal("d1e3a83f642f7b907084394f5f56e89fe7f1196168c2c049d68ae8e986e6c055", actualSha);

            // Spot-check: alpha always 255 (not a transparency channel)
            for (int y = 0; y < 55; y++)
                Assert.Equal((byte)255, decoded[(y * 55 + 0) * 4 + 3]);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    /// <summary>
    /// Decodes frame 0 of F1060 (320x320, full square, no padding).
    /// </summary>
    [Fact]
    public void SyntheticVector_F1060_DecodeFrame0()
    {
        string path = Path.Combine(SyntheticVectorsDir, "F1060_1.ithmb");
        byte[] raw = File.ReadAllBytes(path);
        byte[] frame = raw[..204800];

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1060, Width: 320, Height: 320,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: 204800);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(frame, profile,
                cancellation: null, outInfo, outBuf);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
            Assert.Equal(320, outBuf->Width);
            Assert.Equal(320, outBuf->Height);

            int bgraSize = 320 * 320 * 4;
            var decoded = new ReadOnlySpan<byte>((void*)outBuf->Data, bgraSize);
            string actualSha = Convert.ToHexStringLower(SHA256.HashData(decoded));
            Assert.Equal("4796572fd90cfc75d5b9c3845557fa6d3394856ede24a66c164857324fd72341", actualSha);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    /// <summary>
    /// Decodes frame 0 of F1055 (128x128, full square, no padding).
    /// </summary>
    [Fact]
    public void SyntheticVector_F1055_DecodeFrame0()
    {
        string path = Path.Combine(SyntheticVectorsDir, "F1055_1.ithmb");
        byte[] raw = File.ReadAllBytes(path);
        byte[] frame = raw[..32768];

        var profile = new IthmbCodecPlugin.IthmbVariantProfile(
            Prefix: 1055, Width: 128, Height: 128,
            Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
            FrameByteLength: 32768);

        var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
        var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
            (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
        try
        {
            var status = IthmbCodecPlugin.DecodeRawProfile(frame, profile,
                cancellation: null, outInfo, outBuf);
            Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
            Assert.Equal(128, outBuf->Width);
            Assert.Equal(128, outBuf->Height);

            int bgraSize = 128 * 128 * 4;
            var decoded = new ReadOnlySpan<byte>((void*)outBuf->Data, bgraSize);
            string actualSha = Convert.ToHexStringLower(SHA256.HashData(decoded));
            Assert.Equal("c1cb7d4c0144c2d90a9d4964de8c266a5eebe38bd2b24137c1b24604f139c0aa", actualSha);
        }
        finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
    }

    /// <summary>
    /// Decodes all 10 frames of F1061 and verifies each against manifest.csv SHA256.
    /// Validates our multi-frame slicing handles non-square slot correctly.
    /// </summary>
    [Fact]
    public void SyntheticVector_F1061_AllFrames_Sha256Match()
    {
        string path = Path.Combine(SyntheticVectorsDir, "F1061_1.ithmb");
        byte[] raw = File.ReadAllBytes(path);

        // F1061: 10 frames, each 6160 bytes, slot 56x55
        int frameSize = 6160;
        int frameCount = raw.Length / frameSize;
        Assert.Equal(10, frameCount);

        string[] expectedSha =
        [
            "d1e3a83f642f7b907084394f5f56e89fe7f1196168c2c049d68ae8e986e6c055",
            "a5a86a9e8ca33ab5f2e98c4f4dad1ae0bc93e009ff96e4c590a985927d6541ed",
            "f6a222680e6c043112651c77419c1ac3a3ed5bdb3cd792e50099f417167aa157",
            "c49eefd3bc8f140f9c1224ad012c3020f1ceaf931ce25af51f96431e142cee6b",
            "dc48874385559be1af3d745001976364f22eac286640b2926fde0b55cd918973",
            "efecaa7094431bddbc9e8aff48415e625e0906064b944569c29f8e4e28a0df24",
            "8b7a79323c295a941ef4dacdc6f4fb959a788d1fc4eebdbf9e4943703d6579d5",
            "e4949039d0263de14b6ce9fa8bd089780bdecc6949153b92f57e287233f20156",
            "dfaed3433cbe867cde907b336798be7f844f90f682af4518cdaab1d5e7fcc796",
            "4504dabdacad83c963bc18542bdfb8e9617978aed2e9075e5ad827502955386b",
        ];

        for (int i = 0; i < frameCount; i++)
        {
            var frame = raw.AsSpan(i * frameSize, frameSize).ToArray();
            var profile = new IthmbCodecPlugin.IthmbVariantProfile(
                Prefix: 1061, Width: 55, Height: 55,
                Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
                FrameByteLength: frameSize);

            var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
                (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
            var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
                (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
            try
            {
                var status = IthmbCodecPlugin.DecodeRawProfile(frame, profile,
                    cancellation: null, outInfo, outBuf);
                Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);
                Assert.Equal(55, outBuf->Width);

                int bgraSize = 55 * 55 * 4;
                var decoded = new ReadOnlySpan<byte>((void*)outBuf->Data, bgraSize);
                string actualSha = Convert.ToHexStringLower(SHA256.HashData(decoded));
                Assert.Equal(expectedSha[i], actualSha);
            }
            finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
        }
    }

    /// <summary>
    /// Decodes all 10 frames of F1055 — verifies multi-frame decode for square slots.
    /// </summary>
    [Fact]
    public void SyntheticVector_F1055_AllFrames_Sha256Match()
    {
        string path = Path.Combine(SyntheticVectorsDir, "F1055_1.ithmb");
        byte[] raw = File.ReadAllBytes(path);

        int frameSize = 32768;
        int frameCount = raw.Length / frameSize;
        Assert.Equal(10, frameCount);

        string[] expectedSha =
        [
            "c1cb7d4c0144c2d90a9d4964de8c266a5eebe38bd2b24137c1b24604f139c0aa",
            "bddb75bbd05083b399c10389c31eede1facfa3ed73e0fad88846dae160d272c5",
            "f7532a14c928b8488c8f2bc22e1bf688ec7edfbcbcd5709c6617deee285b0de6",
            "81766dfb7b0284ef659ac46509430f3fbb0b075743e3ce858a205c255322f7c7",
            "1d0d2e70b54c409f5054affe8a5edfa8d1de9c5a030136e0ee8c66fdb4820fe5",
            "1d673f7d2a2fe64d791437e6169e58af6bb02d749dd3ced7b170d6f37bfc5f79",
            "c1cb7d4c0144c2d90a9d4964de8c266a5eebe38bd2b24137c1b24604f139c0aa",
            "bddb75bbd05083b399c10389c31eede1facfa3ed73e0fad88846dae160d272c5",
            "f7532a14c928b8488c8f2bc22e1bf688ec7edfbcbcd5709c6617deee285b0de6",
            "81766dfb7b0284ef659ac46509430f3fbb0b075743e3ce858a205c255322f7c7",
        ];

        for (int i = 0; i < frameCount; i++)
        {
            var frame = raw.AsSpan(i * frameSize, frameSize).ToArray();
            var profile = new IthmbCodecPlugin.IthmbVariantProfile(
                Prefix: 1055, Width: 128, Height: 128,
                Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
                FrameByteLength: frameSize);

            var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
                (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
            var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
                (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
            try
            {
                var status = IthmbCodecPlugin.DecodeRawProfile(frame, profile,
                    cancellation: null, outInfo, outBuf);
                Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);

                int bgraSize = 128 * 128 * 4;
                var decoded = new ReadOnlySpan<byte>((void*)outBuf->Data, bgraSize);
                string actualSha = Convert.ToHexStringLower(SHA256.HashData(decoded));
                Assert.Equal(expectedSha[i], actualSha);
            }
            finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
        }
    }

    /// <summary>
    /// Decodes all 10 frames of F1060 — largest multi-frame test (320x320).
    /// </summary>
    [Fact]
    public void SyntheticVector_F1060_AllFrames_Sha256Match()
    {
        string path = Path.Combine(SyntheticVectorsDir, "F1060_1.ithmb");
        byte[] raw = File.ReadAllBytes(path);

        int frameSize = 204800;
        int frameCount = raw.Length / frameSize;
        Assert.Equal(10, frameCount);

        string[] expectedSha =
        [
            "4796572fd90cfc75d5b9c3845557fa6d3394856ede24a66c164857324fd72341",
            "4891fc8abf1614bb672840938b14cfd4adb6cec12a6a3ee043e7ae5f170ed231",
            "5d1ff0b089cf3222e0b538865f62e3b4a3a50d51ebee4eaf177df7d79fbac221",
            "2363ca842a4927747340a54931cdeb222bc1552a87b41034225d07650242860f",
            "630956d1e10af34a3e5e642ac41d85a7993245205dbaf336babaae080ebdb056",
            "ea2673332384df4593d5d501e1fdbec578700a2e53475a7bbd99a348c7691861",
            "4796572fd90cfc75d5b9c3845557fa6d3394856ede24a66c164857324fd72341",
            "4891fc8abf1614bb672840938b14cfd4adb6cec12a6a3ee043e7ae5f170ed231",
            "5d1ff0b089cf3222e0b538865f62e3b4a3a50d51ebee4eaf177df7d79fbac221",
            "2363ca842a4927747340a54931cdeb222bc1552a87b41034225d07650242860f",
        ];

        for (int i = 0; i < frameCount; i++)
        {
            var frame = raw.AsSpan(i * frameSize, frameSize).ToArray();
            var profile = new IthmbCodecPlugin.IthmbVariantProfile(
                Prefix: 1060, Width: 320, Height: 320,
                Encoding: IthmbCodecPlugin.IthmbEncoding.Rgb565,
                FrameByteLength: frameSize);

            var outInfo = (ImageGlass.SDK.Plugins.IGImageInfo*)NativeMemory.AllocZeroed(
                (nuint)sizeof(ImageGlass.SDK.Plugins.IGImageInfo));
            var outBuf = (ImageGlass.SDK.Plugins.IGPixelBuffer*)NativeMemory.AllocZeroed(
                (nuint)sizeof(ImageGlass.SDK.Plugins.IGPixelBuffer));
            try
            {
                var status = IthmbCodecPlugin.DecodeRawProfile(frame, profile,
                    cancellation: null, outInfo, outBuf);
                Assert.Equal(ImageGlass.SDK.Plugins.IGStatus.OK, status);

                int bgraSize = 320 * 320 * 4;
                var decoded = new ReadOnlySpan<byte>((void*)outBuf->Data, bgraSize);
                string actualSha = Convert.ToHexStringLower(SHA256.HashData(decoded));
                Assert.Equal(expectedSha[i], actualSha);
            }
            finally { NativeMemory.Free(outInfo); NativeMemory.Free(outBuf); }
        }
    }
}
