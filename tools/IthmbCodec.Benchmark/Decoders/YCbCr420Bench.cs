using BenchmarkDotNet.Attributes;
using System.Runtime.InteropServices;

namespace IthmbCodec.Benchmark.Decoders;

[Config(typeof(BenchmarkConfig))]
public unsafe class YCbCr420Bench
{
    private byte[] _src = null!;
    private IntPtr _dst;
    private int _w = 720;
    private int _h = 480;

    [GlobalSetup]
    public void Setup()
    {
        int uvSize = ((_w + 1) / 2) * ((_h + 1) / 2);
        _src = new byte[_w * _h + uvSize * 2];
        Random.Shared.NextBytes(_src);
        _dst = Marshal.AllocHGlobal(_w * _h * 4);
    }

    [GlobalCleanup]
    public void Cleanup() => Marshal.FreeHGlobal(_dst);

    [Benchmark]
    public bool DecodeYcbcr420()
    {
        return IthmbCodecPlugin.DecodeYcbcr420(_src, (byte*)_dst, _w, _h);
    }
}
