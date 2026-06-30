// ABI entry point and static state for the IthmbCodec plugin. Thin orchestration
/*
ImageGlass ITHMB Codec Plugin
Copyright (C) 2026 B67687
MIT License

Reads Apple .ithmb thumbnail-cache files. Primary path: locate an embedded
JPEG payload (JFIF/Exif markers) and decode it via StbImageSharp. Secondary
path: decode known legacy raw thumbnail profiles (RGB565, YUV422, YCbCr420).

Format behavior informed by the IthmbDecoder reference (ImageGlass PR #2316).
This is a clean-room implementation for the v10 native codec plugin ABI.
*/
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using ImageGlass.SDK.Plugins;
using System.Collections.Concurrent;
namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    private static volatile bool _profilesLoaded;

    // Cached host function pointers (set once during init, eliminates pointer chase per call)
    private static volatile delegate* unmanaged[Cdecl]<void*, int> _isCanceledFn;
    private static volatile delegate* unmanaged[Cdecl]<int, IGStringRef, void> _logFn;

    // ------------------------------ Static plugin state ------------------------------
    private static volatile IGPluginApi* _pluginApi;
    private static volatile IGCodecApi* _codecApi;
    private static volatile IGHostApi* _hostApi;


    private static readonly object _initLock = new();
    private static readonly ConcurrentDictionary<nint, byte> _liveBuffers = new();
    // Indexer overwrite is intentional — buffer pointers are unique per NativeMemory.AllocZeroed call.
    // CodecFreePixelBuffer uses TryRemove + null-Data guard for double-free safety.

    // ------------------------------ Entry point ------------------------------
    [UnmanagedCallersOnly(EntryPoint = IGNativeAbi.ENTRY_POINT_NAME, CallConvs = [typeof(CallConvCdecl)])]
    public static IGPluginApi* GetApi(int hostAbiVersion, IGHostApi* hostApi)
    {
        if (hostAbiVersion / 1_000_000 != IGNativeAbi.IG_PLUGIN_ABI_MAJOR) return null;
        if (hostApi == null) return null;
        if (_pluginApi != null) return _pluginApi;
        lock (_initLock)
        {
            if (_pluginApi != null) return _pluginApi;
            _hostApi = hostApi;
            InitStrings();
            InitCodecApi();
            InitPluginApi();
            return _pluginApi;
        }
    }

    // ------------------------------ Plugin API ------------------------------
    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static IGStatus OnInitialize() => IGStatus.OK;

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void OnShutdown()
    {
        FreePluginStrings();
        FreePixelBufferCleanup();
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static IGStatus OnGetCodec(int index, IGCodecApi** outCodecApi)
    {
        if (outCodecApi == null) return IGStatus.InvalidArg;
        if (index != 0) { *outCodecApi = null; return IGStatus.InvalidArg; }
        *outCodecApi = _codecApi;
        return IGStatus.OK;
    }

    // ------------------------------ Codec API ------------------------------

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static IGStatus CodecGetCapability(IGCodecCapability* outCap)
    {
        if (outCap == null) return IGStatus.InvalidArg;
        *outCap = new IGCodecCapability
        {
            CodecId = MakeStringRef(_bufCodecId, CodecIdString.Length),
            CodecName = MakeStringRef(_bufCodecName, CodecNameString.Length),
            MetadataPriority = 90,
            DecodePriority = 90,
            SupportsMetadata = 1,
            SupportsStaticRaster = 1,
            SupportsColorProfiles = 0,
            SupportsAnimation = 0,
            ExtensionCount = SupportedExtensions.Length,
            Extensions = _extArray,
        };
        return IGStatus.OK;
    }


    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static int CodecCanHandleExtension(IGStringRef ext)
    {
        if (ext.Data == null || ext.Length <= 0) return 0;
        var s = new ReadOnlySpan<char>(ext.Data, ext.Length);
        foreach (var supported in SupportedExtensions)
            if (s.Equals(supported, StringComparison.OrdinalIgnoreCase)) return 1;
        return 0;
    }


    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    internal static int CodecCanHandleSignature(byte* signature, int length)
    {
        if (signature == null || length < 8) return 0;
        var span = new ReadOnlySpan<byte>(signature, Math.Min(length, MaxSignatureProbe));
        // SIMD-accelerated search for JPEG SOI marker (FF D8)
        int soi = span.IndexOf(JpegSoiMarker);
        if (soi < 0) return 0;
        // FF D8 must be followed by FF (valid JPEG marker)
        if (soi + 2 >= span.Length || span[soi + 2] != 0xFF) return 0;
        // Verify JFIF or Exif within the scan window (covers marker segments before APP0/APP1)
        int scanEnd = Math.Min(soi + JfifExifScanWindow, span.Length);
        int probeLen = scanEnd - soi - 2;
        if (probeLen <= 0) return 0;
        var probe = span.Slice(soi + 2, probeLen);
        return probe.IndexOf(JfifMarker) >= 0 || probe.IndexOf(ExifMarker) >= 0 ? 1 : 0;
    }


    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static IGStatus CodecLoadMetadata(IGStringRef filePath, IGImageInfo* outInfo, void* cancellation)
    {
        if (outInfo == null) return IGStatus.InvalidArg;
        *outInfo = default;
        return DecodeInternal(filePath, cancellation, outInfo, null);
    }


    /// <summary>Frees a pixel buffer allocated during decode. Thread-safe via ConcurrentDictionary.</summary>
    internal static void FreePixelBuffer(IGPixelBuffer* buf)
    {
        if (buf == null || buf->Data == null) return;
        nint key = (nint)buf->Data;
        if (!_liveBuffers.TryRemove(key, out _)) return;
        NativeMemory.Free((void*)key);
        buf->Data = null;
        buf->ReleaseContext = null;
    }

    [UnmanagedCallersOnly(CallConvs = [typeof(CallConvCdecl)])]
    private static void CodecFreePixelBuffer(IGPixelBuffer* buf) => FreePixelBuffer(buf);

}
