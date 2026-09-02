#![no_main]

use libfuzzer_sys::fuzz_target;
use std::sync::atomic::AtomicBool;

fuzz_target!(|data: &[u8]| {
    let canceled = AtomicBool::new(false);
    let _ = ithmb_core::decode_ithmb(data, &canceled);
});
