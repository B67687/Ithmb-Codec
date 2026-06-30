// String buffer management for the IthmbCodec plugin. Allocates and frees UTF-16
// string buffers for the ImageGlass ABI (char* pointers in IGStringRef structs).
// Separated from the main plugin file for independent compilation and file-size discipline.

using System.Runtime.InteropServices;
using ImageGlass.SDK.Plugins;
using System.Runtime.CompilerServices;

namespace IthmbCodec;

internal static unsafe partial class IthmbCodecPlugin
{
    // String constants used for plugin/codec registration
    private const string PluginIdString = "Plugin_IthmbCodec";
    private const string PluginNameString = "ITHMB Codec";
    private const string VersionString = "1.0.0";
    private const string CodecIdString = "plugin.ithmb.codec";
    private const string CodecNameString = "Apple ITHMB Thumbnail Cache";
    private static readonly string[] SupportedExtensions = [".ithmb"];

    // Allocated string buffers (freed in FreePluginStrings on shutdown)
    private static char* _bufPluginId, _bufPluginName, _bufVersion;
    private static char* _bufCodecId, _bufCodecName;
    private static char** _bufExtensions;
    private static IGStringRef* _extArray;

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    private static IGStringRef MakeStringRef(char* data, int len) => new() { Data = data, Length = len };

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

    private static char* AllocUtf16(string s)
    {
        var buf = (char*)NativeMemory.Alloc((nuint)((s.Length + 1) * sizeof(char)));
        for (var i = 0; i < s.Length; i++) buf[i] = s[i];
        buf[s.Length] = '\0';
        return buf;
    }

    /// <summary>Frees AllocUtf16-allocated string buffers to prevent memory leak on shutdown.</summary>
    private static void FreePluginStrings()
    {
        if (_bufPluginId != null) { NativeMemory.Free(_bufPluginId); _bufPluginId = null; }
        if (_bufPluginName != null) { NativeMemory.Free(_bufPluginName); _bufPluginName = null; }
        if (_bufVersion != null) { NativeMemory.Free(_bufVersion); _bufVersion = null; }
        if (_bufCodecId != null) { NativeMemory.Free(_bufCodecId); _bufCodecId = null; }
        if (_bufCodecName != null) { NativeMemory.Free(_bufCodecName); _bufCodecName = null; }
        if (_bufExtensions != null)
        {
            var count = SupportedExtensions.Length;
            for (var i = 0; i < count; i++)
                if (_bufExtensions[i] != null) { NativeMemory.Free(_bufExtensions[i]); _bufExtensions[i] = null; }
            NativeMemory.Free(_bufExtensions); _bufExtensions = null;
        }
        if (_extArray != null) { NativeMemory.Free(_extArray); _extArray = null; }
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
}
