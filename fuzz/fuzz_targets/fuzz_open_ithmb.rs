#![no_main]

use libfuzzer_sys::fuzz_target;
use std::sync::atomic::AtomicBool;

fuzz_target!(|data: &[u8]| {
    let canceled = AtomicBool::new(false);
    // The open_ithmb path exercises both PhotoDB container parsing
    // and raw .ithmb blob decoding, depending on input content.
    let _ = ithmb_core::open_ithmb(data, &canceled, None);
});
