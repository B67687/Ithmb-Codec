using BenchmarkDotNet.Attributes;
using System.Runtime.InteropServices;

namespace IthmbCodec.Benchmark.Decoders;

[Config(typeof(BenchmarkConfig))]
public unsafe class Yuv422InterlacedBench
{
    private byte[] _src = null!;
    private IntPtr _dst;
    private int _w = 720;
    private int _h = 480;

    [GlobalSetup]
    public void Setup()
    {
        _src = new byte[_w * _h * 2];
        Random.Shared.NextBytes(_src);
        _dst = Marshal.AllocHGlobal(_w * _h * 4);
    }

    [GlobalCleanup]
    public void Cleanup() => Marshal.FreeHGlobal(_dst);

    [Benchmark]
    public bool DecodeYuv422Interlaced()
    {
        return IthmbCodecPlugin.DecodeYuv422Interlaced(_src, (byte*)_dst, _w, _h);
    }
}
