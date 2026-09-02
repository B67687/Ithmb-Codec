#![no_main]

use ithmb_core::profile_parser::parse_profiles_json;
use libfuzzer_sys::fuzz_target;

/// Fuzz target: parse arbitrary bytes as a profiles.json document.
///
/// The hand-rolled JSON parser (`profile_parser.rs`) is a weak module for
/// mutation testing (string escapes, hex4, i32 number scanning). This target
/// feeds it arbitrary bytes via a lossy UTF-8 conversion and must never panic,
/// hang, or allocate from attacker-controlled sizes. Parse errors are expected
/// fuzz outcomes and are ignored.
fuzz_target!(|data: &[u8]| {
    // Lossy conversion keeps the parser seeing well-formed UTF-8 while still
    // preserving arbitrary byte structure for the escape/number scanners.
    let input = String::from_utf8_lossy(data);
    let _ = parse_profiles_json(&input);
});
