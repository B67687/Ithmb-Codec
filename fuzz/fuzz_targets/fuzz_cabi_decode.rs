#![no_main]

use libfuzzer_sys::fuzz_target;
use std::sync::atomic::AtomicBool;

/// Fuzz the C ABI entry point exercised by the decode pipeline.
///
/// This target calls the codec's internal decode function via the pipeline,
/// mirroring what the C ABI `codec_decode_static_raster` does after reading
/// a file: it invokes `ithmb_core::decode_ithmb` on the raw fuzz bytes.
fuzz_target!(|data: &[u8]| {
    let canceled = AtomicBool::new(false);
    let _ = ithmb_core::decode_ithmb(data, &canceled);
});
