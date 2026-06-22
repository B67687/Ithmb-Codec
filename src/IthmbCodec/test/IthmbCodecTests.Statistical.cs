using System.Runtime.InteropServices;
using IthmbCodec;
using Xunit;

namespace IthmbCodec.Tests;

public unsafe partial class IthmbCodecTests
{
    private const int FuzzTrials = 10_000;
    private static readonly int[] FuzzSizes = [2, 4, 6, 8, 10, 16, 32];
}
