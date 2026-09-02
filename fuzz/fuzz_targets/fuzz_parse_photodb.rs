#![no_main]

use ithmb_core::photodb::parser::try_parse_photodb;
use libfuzzer_sys::fuzz_target;

/// Fuzz target: parse arbitrary bytes as a PhotoDB/ArtworkDB chunk tree.
///
/// `try_parse_photodb` walks an untrusted binary chunk tree (MHFD → MHSD →
/// MHNI...) with endianness detection, saturating-add bounds checks, and a
/// depth cap. This target must never panic or allocate from attacker-controlled
/// chunk sizes. Parse errors are expected fuzz outcomes and are ignored.
fuzz_target!(|data: &[u8]| {
    let mut entries: Vec<ithmb_core::photodb::parser::PhotoDbEntry> = Vec::new();
    let _ = try_parse_photodb(data, &mut entries);
});
