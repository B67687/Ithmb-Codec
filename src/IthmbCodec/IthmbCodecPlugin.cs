// SIZE_OK: ABI skeleton + shared decode utilities (~380 LOC)
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
using System.Buffers.Binary;
using System.Collections.Concurrent;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using ImageGlass.SDK.Plugins;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // ------------------------------ Constants ------------------------------
    private const string PluginIdString = "Plugin_IthmbCodec";
    private const string PluginNameString = "ITHMB Codec";
    private const string VersionString = "1.0.0";
    private const string CodecIdString = "plugin.ithmb.codec";
    private const string CodecNameString = "Apple ITHMB Thumbnail Cache";

    private static readonly string[] SupportedExtensions = [".ithmb"];

    // JPEG markers for embedded payload detection
    private static readonly byte[] JfifMarker = "JFIF\0"u8.ToArray();
    private static readonly byte[] ExifMarker = "Exif\0\0"u8.ToArray();
    private static readonly byte[] JpegSoiMarker = [0xFF, 0xD8];
    internal static readonly byte[] JpegEoiMarker = [0xFF, 0xD9];
    private static readonly byte[] App1Marker = [0xFF, 0xE1];

    // Size limit: prevents OOM/DoS on corrupt/malicious files. Largest single raw
    // Largest single frame: 829 KB (P1007 480×864). Largest real .ithmb observed:
    // 852 KB (T-prefix JPEG). No .ithmb file > 1 MB exists in any public repo.
    // Multi-frame concatenation: worst-case 40 frames of the largest profile fit
    // within 32 MB. This covers all plausible real-world usage with generous margin.
    // See research notes in  for the full evidence chain.
    internal const long MaxDecodeFileSize = 32L * 1024 * 1024;
    private const int PeekBufferSize = 4 * 1024 * 1024;           // 4 MB: covers thumbnail JPEG headers + embedded JPEGs
    private const int MaxSignatureProbe = 4096;                    // 4 KB: covers JPEG SOI + marker segments

    // Tolerance for trailing alignment padding bytes. Real .ithmb files from some devices
    // may be slightly smaller than FrameByteLength due to device alignment quirks or
    // incomplete padding. Allow up to 256 bytes slack before rejecting as too small.
    // Based on analysis of iOpenPod's _resolve_packed_geometry trailing-trim approach.
    private const int TrailingPaddingTolerance = 256;

    // JFIF/Exif probe window after SOI (must cover DQT, DHT, COM before APP0/APP1)
    private const int JfifExifScanWindow = 512;

    private static volatile bool _profilesLoaded;

    // Cached host function pointers (set once during init, eliminates pointer chase per call)
    private static volatile delegate* unmanaged[Cdecl]<void*, int> _isCanceledFn;
    private static volatile delegate* unmanaged[Cdecl]<int, IGStringRef, void> _logFn;

    // ------------------------------ Static plugin state ------------------------------
    private static volatile IGPluginApi* _pluginApi;
    private static volatile IGCodecApi* _codecApi;
    private static volatile IGHostApi* _hostApi;

    private static char* _bufPluginId, _bufPluginName, _bufVersion;
    private static char* _bufCodecId, _bufCodecName;
    private static char** _bufExtensions;
    private static IGStringRef* _extArray;

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
    private static void OnShutdown() { }

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

    // ------------------------------ Helpers ------------------------------

    /// <summary>Rotates a BGRA pixel buffer in place (90, 180, or 270 degrees clockwise). Updates w/h.</summary>
    internal static void RotateBgra(byte* pixels, ref int w, ref int h, int rotation)
    {
        if (rotation == 0) return;
        int srcW = w, srcH = h;
        int pixelCount = srcW * srcH;

        // 180°: simple in-place swap of opposite pixels
        if (rotation == 180)
        {
            for (int i = 0; i < pixelCount / 2; i++)
            {
                int j = pixelCount - 1 - i;
                for (int c = 0; c < 4; c++)
                {
                    (pixels[j * 4 + c], pixels[i * 4 + c]) = (pixels[i * 4 + c], pixels[j * 4 + c]);
                }
            }
            return;
        }

        // 90° or 270°: allocate temp buffer, rotate, copy back
        int dstW = srcH, dstH = srcW;
        int newSize = dstW * dstH * 4;
        byte* rotated = (byte*)NativeMemory.Alloc((nuint)newSize);
        if (rotated == null) return; // OOM: skip rotation
        try
        {
            for (int y = 0; y < srcH; y++)
            {
                for (int x = 0; x < srcW; x++)
                {
                    int srcIdx = (y * srcW + x) * 4;
                    int dstIdx = rotation == 90
                        ? (x * srcH + (srcH - 1 - y)) * 4
                        : ((srcW - 1 - x) * srcH + y) * 4;

                    rotated[dstIdx]     = pixels[srcIdx];
                    rotated[dstIdx + 1] = pixels[srcIdx + 1];
                    rotated[dstIdx + 2] = pixels[srcIdx + 2];
                    rotated[dstIdx + 3] = pixels[srcIdx + 3];
                }
            }
            NativeMemory.Copy(rotated, pixels, (nuint)newSize);
            w = dstW;
            h = dstH;
        }
        finally
        {
            NativeMemory.Free(rotated);
        }
    }

    /// <summary>Populates an IGImageInfo with common defaults.</summary>
    private static void FillImageInfo(IGImageInfo* info, int w, int h, int hasAlpha, int orientation, long fileSize = -1, int frameCount = 1)
    {
        info->Width = w;
        info->Height = h;
        info->PixelFormat = (int)IGPixelFormat.Bgra8Unorm;
        info->HasAlpha = hasAlpha;
        info->HdrTransferFn = (int)IGHdrTransferFn.None;
        info->ColorSpace = (int)IGColorSpace.Srgb;
        info->Orientation = orientation;
        info->FrameCount = frameCount;
        info->FileSizeBytes = fileSize;
        info->IccProfileData = null;
        info->IccProfileSize = 0;
    }

    /// <summary>Allocates a BGRA8 pixel buffer; returns OOM status on failure.</summary>
    private static IGStatus AllocateBgraBuffer(int w, int h, out ulong stride, out byte* pixels)
    {
        stride = (ulong)w * 4UL;
        ulong size = stride * (ulong)h;
        if (size > int.MaxValue) { pixels = null; return IGStatus.OutOfMemory; }
        pixels = (byte*)NativeMemory.AllocZeroed((nuint)size);
        if (pixels == null) return IGStatus.OutOfMemory;
        return IGStatus.OK;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static int IndexOf(byte[] haystack, byte[] needle, int start, int end)
    {
        int len = end - start;
        return len <= 0 ? -1 : haystack.AsSpan(start, len).IndexOf(needle);
    }

    /// <summary>
    /// Reads the EXIF Orientation tag (0x0112) from a JPEG slice.
    /// Returns 1-8 on success, or 1 (normal) if not found.
    /// </summary>
    internal static int ReadExifOrientation(byte[] data, int jpegOffset, int jpegLength)
    {
        int end = jpegOffset + jpegLength;
        var jpeg = data.AsSpan(jpegOffset, jpegLength);

        // SIMD-accelerated search for APP1 marker (FF E1)
        int app1Rel = jpeg.IndexOf(App1Marker);
        if (app1Rel < 0) return 1;
        int app1Start = jpegOffset + app1Rel;

        // APP1 segment: FF E1 len_len (big-endian 16-bit length including self)
        if (app1Start + 4 >= end) return 1;
        int segEnd = app1Start + 2 + BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(app1Start + 2));
        if (segEnd > end) return 1;

        // Look for "Exif\0\0" header within APP1
        int exifOff = app1Start + 4;
        if (exifOff + 6 > end) return 1;
        if (data[exifOff] != 'E' || data[exifOff + 1] != 'x' ||
            data[exifOff + 2] != 'i' || data[exifOff + 3] != 'f' ||
            data[exifOff + 4] != 0 || data[exifOff + 5] != 0) return 1;

        // TIFF header: "II" (little-endian) or "MM" (big-endian)
        int tiffStart = exifOff + 6;
        if (tiffStart + 8 > end) return 1;
        bool le = data[tiffStart] == 'I' && data[tiffStart + 1] == 'I';
        bool be = data[tiffStart] == 'M' && data[tiffStart + 1] == 'M';
        if (!le && !be) return 1;

        // TIFF magic: 0x002A
        if ((le ? BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(tiffStart + 2))
                : BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(tiffStart + 2))) != 0x002A) return 1;

        // IFD0 offset
        int ifdOff = tiffStart + 4;
        int ifdPos = tiffStart + (int)(le ? BinaryPrimitives.ReadUInt32LittleEndian(data.AsSpan(ifdOff))
                                          : BinaryPrimitives.ReadUInt32BigEndian(data.AsSpan(ifdOff)));
        if (ifdPos < tiffStart + 8 || ifdPos + 2 > end) return 1;

        // Number of IFD entries (16-bit)
        int numEntries = le ? BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(ifdPos))
                           : BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(ifdPos));

        // Scan IFD for Orientation tag (0x0112)
        int entryStart = ifdPos + 2;
        for (int e = 0; e < Math.Min(numEntries, 100) && entryStart + 12 <= end; e++, entryStart += 12)
        {
            int tag = le ? BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(entryStart))
                        : BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(entryStart));
            if (tag != 0x0112) continue;
            // Type must be SHORT (3), count 1
            int type = le ? BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(entryStart + 2))
                         : BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(entryStart + 2));
            int count = (int)(le ? BinaryPrimitives.ReadUInt32LittleEndian(data.AsSpan(entryStart + 4))
                                : BinaryPrimitives.ReadUInt32BigEndian(data.AsSpan(entryStart + 4)));
            if (type != 3 || count != 1) continue;
            // Orientation value is in the last 2 bytes (SHORT fits in 2 bytes)
            int orient = le ? BinaryPrimitives.ReadUInt16LittleEndian(data.AsSpan(entryStart + 8))
                           : BinaryPrimitives.ReadUInt16BigEndian(data.AsSpan(entryStart + 8));
            return orient is >= 1 and <= 8 ? orient : 1;
        }
        return 1;
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static bool IsCanceled(void* cancellation)
    {
        // Use cached function pointer (set during init). Avoids chasing _hostApi->Core->{fn} per call.
        return cancellation != null && _isCanceledFn != null && _isCanceledFn(cancellation) != 0;
    }

    private static void Log(int level, string message)
    {
        if (_logFn == null) return;
        fixed (char* p = message) _logFn(level, new IGStringRef { Data = p, Length = message.Length });
    }

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static IGStringRef MakeStringRef(char* data, int len) => new() { Data = data, Length = len };

    // ------------------------------ Init ------------------------------
    private static void InitStrings()
    {
        _bufPluginId = AllocUtf16(PluginIdString);
        _bufPluginName = AllocUtf16(PluginNameString);
        _bufVersion = AllocUtf16(VersionString);
        _bufCodecId = AllocUtf16(CodecIdString);
        _bufCodecName = AllocUtf16(CodecNameString);

        var count = SupportedExtensions.Length;
        _bufExtensions = (char**)NativeMemory.AllocZeroed((nuint)(sizeof(nint) * count));
        _extArray = (IGStringRef*)NativeMemory.AllocZeroed((nuint)(sizeof(IGStringRef) * count));
        if (_bufExtensions == null || _extArray == null) return;
        for (var i = 0; i < count; i++)
        {
            var ext = SupportedExtensions[i];
            _bufExtensions[i] = AllocUtf16(ext);
            _extArray[i] = MakeStringRef(_bufExtensions[i], ext.Length);
        }

        // Cache host function pointers for fast-path access
        if (_hostApi != null && _hostApi->Core != null)
        {
            _isCanceledFn = _hostApi->Core->IsCancellationRequested;
            _logFn = _hostApi->Core->Log;
        }
    }

    private static void InitCodecApi()
    {
        _codecApi = (IGCodecApi*)NativeMemory.AllocZeroed((nuint)sizeof(IGCodecApi));
        _codecApi->GetCapability = &CodecGetCapability;
        _codecApi->CanHandleExtension = &CodecCanHandleExtension;
        _codecApi->CanHandleSignature = &CodecCanHandleSignature;
        _codecApi->LoadMetadata = &CodecLoadMetadata;
        _codecApi->DecodeStaticRaster = &CodecDecodeStaticRaster;
        _codecApi->FreePixelBuffer = &CodecFreePixelBuffer;
        _codecApi->GetAnimationInfo = null;
        _codecApi->FreeAnimationInfo = null;
        _codecApi->DecodeAnimationFrame = null;
    }

    private static void InitPluginApi()
    {
        _pluginApi = (IGPluginApi*)NativeMemory.AllocZeroed((nuint)sizeof(IGPluginApi));
        _pluginApi->StructSize = sizeof(IGPluginApi);
        _pluginApi->AbiVersion = IGNativeAbi.IG_PLUGIN_ABI_VERSION;
        _pluginApi->Info = new IGPluginInfo
        {
            PluginId = MakeStringRef(_bufPluginId, PluginIdString.Length),
            Name = MakeStringRef(_bufPluginName, PluginNameString.Length),
            Version = MakeStringRef(_bufVersion, VersionString.Length),
            AbiVersion = IGNativeAbi.IG_PLUGIN_ABI_VERSION,
            CodecCount = 1,
        };
        _pluginApi->GetCodec = &OnGetCodec;
        _pluginApi->Initialize = &OnInitialize;
        _pluginApi->Shutdown = &OnShutdown;
        _pluginApi->SelfTest = null;
    }

    private static char* AllocUtf16(string s)
    {
        var buf = (char*)NativeMemory.Alloc((nuint)((s.Length + 1) * sizeof(char)));
        for (var i = 0; i < s.Length; i++) buf[i] = s[i];
        buf[s.Length] = '\0';
        return buf;
    }
}
