#![allow(
    unused_crate_dependencies,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::pedantic,
    clippy::unwrap_used
)]

//! PhotoDB encode→decode→compare roundtrip integration test.
//!
//! Builds a synthetic PhotoDB container with 3 entries (Rgb565, Rgb555, Yuv422),
//! serialises, parses back, and verifies pixel data and metadata preservation.
//! Tests both little-endian and big-endian output.
//!
//! Coverage:
//!   - LE roundtrip (build → parse → verify entries + decode)
//!   - BE roundtrip (build → fix magics → parse → verify)
//!   - `open_ithmb` path on a PhotoDB container
//!   - MHOD/MHIF metadata injection and preservation

use std::sync::atomic::AtomicBool;

use ithmb_core::enc::encode_bgra;
use ithmb_core::photodb::builder::{try_build_photodb, BuildEntry};
use ithmb_core::photodb::parser::{try_parse_photodb, PhotoDbEntry, PhotoDbEntryKind};
use ithmb_core::pipeline::{decode_with_profile, open_ithmb};
use ithmb_core::profile::Profile;
use ithmb_core::profile_db::ProfileDb;
use ithmb_core::DecodedImage;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_profile(format_id: i32) -> Profile {
    ProfileDb::load_builtin()
        .expect("built-in profile DB should load")
        .get(format_id)
        .expect("format ID should exist in DB")
        .clone()
}

/// Create a deterministic BGRA test pattern with unique per-channel multipliers
/// so that byte-order bugs (e.g. BGR↔RGB) are immediately visible.
fn make_bgra(w: i32, h: i32, seed: u8) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let v = (x.wrapping_mul(51) ^ y.wrapping_mul(37)).wrapping_add(i32::from(seed)) as u8;
            pixels.push(v.wrapping_mul(3)); // B
            pixels.push(v.wrapping_mul(7)); // G
            pixels.push(v.wrapping_mul(11)); // R
            pixels.push(255); // A
        }
    }
    pixels
}

/// Encode BGRA data into a [`BuildEntry`] using the given profile.
///
/// # Panics
/// Panics if the encoded data length does not match `profile.frame_byte_length`.
fn build_entry(profile: &Profile, bgra: &[u8]) -> BuildEntry {
    let encoded = encode_bgra(bgra, profile.width, profile.height, profile);
    assert_eq!(
        encoded.len() as i32,
        profile.frame_byte_length,
        "encoded length mismatch for format_id={}",
        profile.prefix,
    );
    BuildEntry {
        format_id: profile.prefix,
        data: encoded,
    }
}

// ---------------------------------------------------------------------------
// Inline byte-level writers (crate-internal helpers are `pub(crate)`)
// ---------------------------------------------------------------------------

#[allow(clippy::cast_possible_truncation)]
fn write_u32_le(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset] = value as u8;
    buf[offset + 1] = (value >> 8) as u8;
    buf[offset + 2] = (value >> 16) as u8;
    buf[offset + 3] = (value >> 24) as u8;
}

#[allow(clippy::cast_possible_truncation)]
fn write_u32_be(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset] = (value >> 24) as u8;
    buf[offset + 1] = (value >> 16) as u8;
    buf[offset + 2] = (value >> 8) as u8;
    buf[offset + 3] = value as u8;
}

#[allow(clippy::cast_possible_truncation)]
fn write_u16_le(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset] = value as u8;
    buf[offset + 1] = (value >> 8) as u8;
}

#[allow(clippy::cast_possible_truncation)]
fn write_u16_be(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset] = (value >> 8) as u8;
    buf[offset + 1] = value as u8;
}

// ---------------------------------------------------------------------------
// BE magic fixup — byte-swap LE-ASCII magics to BE byte-swapped form
// ---------------------------------------------------------------------------

/// Byte-swap magic constants from LE (ASCII `"mhfd"`) to BE (`"dfhm"`).
///
/// The builder always writes LE-ASCII magic bytes regardless of the
/// `big_endian` flag, so we post-process to produce a buffer that
/// [`try_parse_photodb`] recognises as big-endian.
fn fix_be_magics(buf: &mut [u8], num_entries: usize, mhni_header_size: i32, mhni_padding_size: i32) {
    let mhni_total_len = (mhni_header_size + mhni_padding_size) as usize;
    let mhii_total_len = 12 + mhni_total_len;
    let mhii_start: usize = 40; // MHFD(12) + MHSD(16) + MHL(12)

    // All known magic offsets in the builder's output layout.
    let mut offsets = vec![0usize, 12, 28]; // MHFD, MHSD, MHL
    for i in 0..num_entries {
        offsets.push(mhii_start + i * mhii_total_len); // MHII
        offsets.push(mhii_start + i * mhii_total_len + 12); // MHNI
    }

    const BE_MAGICS: &[(&[u8; 4], [u8; 4])] = &[
        (b"mhfd", [0x64, 0x66, 0x68, 0x6d]), // "dfhm"
        (b"mhsd", [0x64, 0x73, 0x68, 0x6d]), // "dshm"
        (b"mhli", [0x69, 0x6c, 0x68, 0x6d]), // "ilhm"
        (b"mhii", [0x69, 0x69, 0x68, 0x6d]), // "iihm"
        (b"mhni", [0x69, 0x6e, 0x68, 0x6d]), // "inhm"
    ];

    for off in offsets {
        if off + 4 > buf.len() {
            continue;
        }
        let le_magic: &[u8; 4] = (&buf[off..off + 4]).try_into().expect("4-byte slice");
        if let Some(be_encoded) = BE_MAGICS.iter().find(|(le, _)| *le == le_magic).map(|(_, be)| *be) {
            buf[off..off + 4].copy_from_slice(&be_encoded);
        }
    }
}

// ---------------------------------------------------------------------------
// MHOD/MHIF injection into padding area
// ---------------------------------------------------------------------------

/// Inject MHOD (album name string) and MHIF (info data) chunks into the
/// padding region of a specific entry's MHNI slot.
///
/// The builder reserves `mhni_padding_size` bytes after each MHNI header.
/// We overwrite those zeros with valid MHOD+MHIF chunks, which the parser
/// picks up as children of the enclosing MHII container.
fn inject_mhod_mhif(
    buf: &mut [u8],
    entry_index: usize,
    mhni_header_size: i32,
    mhni_padding_size: i32,
    big_endian: bool,
) {
    let mhni_total_len = (mhni_header_size + mhni_padding_size) as usize;
    let mhii_total_len = 12 + mhni_total_len;
    let mhii_start: usize = 40;

    let mhni_pos = mhii_start + entry_index * mhii_total_len + 12;
    let mut cursor = mhni_pos + mhni_header_size as usize;

    let wu32 = if big_endian { write_u32_be } else { write_u32_le };
    let wu16 = if big_endian { write_u16_be } else { write_u16_le };

    // ── MHOD: album name string ──
    let album = b"MyAlbum\0";
    let mhod_tag: u16 = 1;
    let mhod_str_size: u16 = album.len() as u16;
    let mhod_chunk_size: u32 = 8 + 4 + u32::from(mhod_str_size); // magic(4) + header_size(4) + MhodHeader(4) + data

    if big_endian {
        buf[cursor..cursor + 4].copy_from_slice(&[0x64, 0x6f, 0x68, 0x6d]); // "dohm"
    } else {
        buf[cursor..cursor + 4].copy_from_slice(b"mhod");
    }
    cursor += 4;
    wu32(buf, cursor, mhod_chunk_size);
    cursor += 4;
    wu16(buf, cursor, mhod_tag);
    cursor += 2;
    wu16(buf, cursor, mhod_str_size);
    cursor += 2;
    buf[cursor..cursor + album.len()].copy_from_slice(album);
    cursor += album.len();

    // ── MHIF: info block (12-byte header, no extra data) ──
    if big_endian {
        buf[cursor..cursor + 4].copy_from_slice(&[0x66, 0x69, 0x68, 0x6d]); // "fihm"
    } else {
        buf[cursor..cursor + 4].copy_from_slice(b"mhif");
    }
    cursor += 4;
    wu32(buf, cursor, 12); // header_size = 12 (magic+header_size+info_type)
    cursor += 4;
    wu32(buf, cursor, 42); // info_type (arbitrary test value)
    cursor += 4;

    let pad_end = mhni_pos + mhni_total_len;
    assert!(
        cursor <= pad_end,
        "MHOD+MHIF overflow: cursor={cursor} > pad_end={pad_end}",
    );
}

// ---------------------------------------------------------------------------
// Tolerance comparison
// ---------------------------------------------------------------------------

/// Assert that two BGRA buffers match within a per-channel tolerance.
fn assert_bgra_tolerant(actual: &[u8], expected: &[u8], tolerance: u8, label: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{label}: length mismatch ({} vs {})",
        actual.len(),
        expected.len(),
    );
    for (px, (a_chunk, e_chunk)) in actual.chunks_exact(4).zip(expected.chunks_exact(4)).enumerate() {
        for c in 0..3 {
            let diff = (i16::from(a_chunk[c]) - i16::from(e_chunk[c])).unsigned_abs();
            assert!(
                diff <= u16::from(tolerance),
                "{label}: pixel {px} channel {c}: diff={diff}, got={}, expected={}",
                a_chunk[c],
                e_chunk[c],
            );
        }
        assert_eq!(a_chunk[3], 255, "{label}: pixel {px} alpha");
    }
}

// ---------------------------------------------------------------------------
// Entry verification helper
// ---------------------------------------------------------------------------

fn verify_entry(
    parsed: &PhotoDbEntry,
    original: &BuildEntry,
    profile: &Profile,
    expected_decoded: &DecodedImage,
    label: &str,
) {
    assert_eq!(parsed.format_id, original.format_id, "{label}: format_id");
    assert_eq!(parsed.width, profile.width, "{label}: width");
    assert_eq!(parsed.height, profile.height, "{label}: height");
    assert_eq!(parsed.data, original.data, "{label}: pixel data");
    assert_eq!(parsed.image_size, original.data.len() as i32, "{label}: image_size",);
    assert_eq!(parsed.kind, PhotoDbEntryKind::Inline, "{label}: kind");

    // Decode the parsed data — the result must match the expected decode.
    let canceled = AtomicBool::new(false);
    let mut with_prefix = Vec::with_capacity(4 + parsed.data.len());
    with_prefix.extend_from_slice(&(profile.prefix as u32).to_be_bytes());
    with_prefix.extend_from_slice(&parsed.data);
    let decoded =
        decode_with_profile(&with_prefix, profile, &canceled).unwrap_or_else(|e| panic!("{label}: decode failed: {e}"));

    assert_eq!(decoded.width as i32, profile.width, "{label}: decoded width",);
    assert_eq!(decoded.height as i32, profile.height, "{label}: decoded height",);

    // Lossy formats (Yuv422) have chroma-subsampling error;
    // Rgb565/Rgb555 have MSB-replication error within ±8.
    // Compare against the expected-roundtrip (not raw BGRA) to avoid
    // false failures from encode→decode loss that is outside our control.
    let tolerance = if profile.encoding == ithmb_core::profile::Encoding::Yuv422 {
        16u8
    } else {
        8u8
    };
    assert_bgra_tolerant(&decoded.data, &expected_decoded.data, tolerance, label);
}

/// Decode raw encoded data (with prefix) to get the reference DecodedImage.
fn decode_reference(entry: &BuildEntry, profile: &Profile) -> DecodedImage {
    let canceled = AtomicBool::new(false);
    let mut with_prefix = Vec::with_capacity(4 + entry.data.len());
    with_prefix.extend_from_slice(&(profile.prefix as u32).to_be_bytes());
    with_prefix.extend_from_slice(&entry.data);
    decode_with_profile(&with_prefix, profile, &canceled).expect("reference decode should succeed")
}
// ===========================================================================
// Tests
// ===========================================================================

#[test]
fn le_roundtrip() {
    // Three formats: Rgb565, Rgb555 (padded), Yuv422 (interlaced).
    let p1 = get_profile(1016); // Rgb565, 140×140
    let p2 = get_profile(3004); // Rgb555, 56×55, padded slot
    let p3 = get_profile(1019); // Yuv422, 720×480, interlaced

    let bgra1 = make_bgra(p1.width, p1.height, 0);
    let bgra2 = make_bgra(p2.width, p2.height, 42);
    let bgra3 = make_bgra(p3.width, p3.height, 99);

    let e1 = build_entry(&p1, &bgra1);
    let e2 = build_entry(&p2, &bgra2);
    let e3 = build_entry(&p3, &bgra3);

    // Pre-compute decode references (ground truth for encode→decode).
    let ref1 = decode_reference(&e1, &p1);
    let ref2 = decode_reference(&e2, &p2);
    let ref3 = decode_reference(&e3, &p3);

    let entries = vec![e1, e2, e3];
    let data = try_build_photodb(&entries, 36, 40, false).expect("LE build should succeed");
    let mut parsed = Vec::new();
    try_parse_photodb(&data, &mut parsed).expect("LE parse should succeed");

    assert_eq!(parsed.len(), 3, "LE: should have 3 entries");

    verify_entry(&parsed[0], &entries[0], &p1, &ref1, "LE entry 0 (1016)");
    verify_entry(&parsed[1], &entries[1], &p2, &ref2, "LE entry 1 (3004)");
    verify_entry(&parsed[2], &entries[2], &p3, &ref3, "LE entry 2 (1019)");
}

#[test]
fn be_roundtrip() {
    let p1 = get_profile(1016);
    let p2 = get_profile(3004);
    let p3 = get_profile(1019);

    let bgra1 = make_bgra(p1.width, p1.height, 10);
    let bgra2 = make_bgra(p2.width, p2.height, 20);
    let bgra3 = make_bgra(p3.width, p3.height, 30);

    let e1 = build_entry(&p1, &bgra1);
    let e2 = build_entry(&p2, &bgra2);
    let e3 = build_entry(&p3, &bgra3);

    let ref1 = decode_reference(&e1, &p1);
    let ref2 = decode_reference(&e2, &p2);
    let ref3 = decode_reference(&e3, &p3);

    let entries = vec![e1, e2, e3];
    let mut data = try_build_photodb(&entries, 36, 40, true).expect("BE build should succeed");
    fix_be_magics(&mut data, entries.len(), 36, 40);

    let mut parsed = Vec::new();
    try_parse_photodb(&data, &mut parsed).expect("BE parse should succeed");

    assert_eq!(parsed.len(), 3, "BE: should have 3 entries");

    verify_entry(&parsed[0], &entries[0], &p1, &ref1, "BE entry 0 (1016)");
    verify_entry(&parsed[1], &entries[1], &p2, &ref2, "BE entry 1 (3004)");
    verify_entry(&parsed[2], &entries[2], &p3, &ref3, "BE entry 2 (1019)");
}

#[test]
fn open_ithmb_roundtrip() {
    let p1 = get_profile(1016);
    let p2 = get_profile(3004);
    let p3 = get_profile(1019);

    let bgra1 = make_bgra(p1.width, p1.height, 0);
    let bgra2 = make_bgra(p2.width, p2.height, 42);
    let bgra3 = make_bgra(p3.width, p3.height, 99);

    let e1 = build_entry(&p1, &bgra1);
    let e2 = build_entry(&p2, &bgra2);
    let e3 = build_entry(&p3, &bgra3);

    let ref1 = decode_reference(&e1, &p1);
    let ref2 = decode_reference(&e2, &p2);
    let ref3 = decode_reference(&e3, &p3);

    let entries = vec![e1, e2, e3];
    let data = try_build_photodb(&entries, 36, 40, false).expect("build for open_ithmb");

    let canceled = AtomicBool::new(false);
    let results = open_ithmb(&data, &canceled, None).expect("open_ithmb should succeed");

    assert_eq!(results.len(), 3, "open_ithmb: should return 3 images");

    let refs = [&ref1, &ref2, &ref3];
    let profiles = [&p1, &p2, &p3];
    for i in 0..3 {
        assert_eq!(
            results[i].width as i32, profiles[i].width,
            "open_ithmb image {i}: width",
        );
        assert_eq!(
            results[i].height as i32, profiles[i].height,
            "open_ithmb image {i}: height",
        );
        let tolerance = if profiles[i].encoding == ithmb_core::profile::Encoding::Yuv422 {
            16u8
        } else {
            8u8
        };
        assert_bgra_tolerant(
            &results[i].data,
            &refs[i].data,
            tolerance,
            &format!("open_ithmb image {i}"),
        );
    }
}

#[test]
fn metadata_roundtrip() {
    let p1 = get_profile(1016);
    let p2 = get_profile(3004);
    let p3 = get_profile(1019);

    let bgra1 = make_bgra(p1.width, p1.height, 0);
    let bgra2 = make_bgra(p2.width, p2.height, 42);
    let bgra3 = make_bgra(p3.width, p3.height, 99);

    let e1 = build_entry(&p1, &bgra1);
    let e2 = build_entry(&p2, &bgra2);
    let e3 = build_entry(&p3, &bgra3);

    let entries = vec![e1, e2, e3];

    // ── LE with injected metadata ──
    {
        let mut data = try_build_photodb(&entries, 36, 200, false).expect("LE build (metadata)");
        for i in 0..entries.len() {
            inject_mhod_mhif(data.as_mut_slice(), i, 36, 200, false);
        }
        let mut parsed = Vec::new();
        try_parse_photodb(&data, &mut parsed).expect("LE parse (metadata)");
        assert_eq!(parsed.len(), 3, "LE metadata: 3 entries");
        for i in 0..3 {
            assert!(
                !parsed[i].metadata.mhod_strings.is_empty(),
                "LE entry {i}: should have MHOD strings",
            );
            assert_eq!(
                parsed[i].metadata.mhod_strings[0], "MyAlbum",
                "LE entry {i}: album name",
            );
            assert_eq!(parsed[i].metadata.mhif_info_type, Some(42), "LE entry {i}: info type",);
            assert_eq!(parsed[i].data, entries[i].data, "LE entry {i}: pixel data preserved",);
        }
    }

    // ── BE with injected metadata ──
    {
        let mut data = try_build_photodb(&entries, 36, 200, true).expect("BE build (metadata)");
        fix_be_magics(&mut data, entries.len(), 36, 200);
        for i in 0..entries.len() {
            inject_mhod_mhif(data.as_mut_slice(), i, 36, 200, true);
        }
        let mut parsed = Vec::new();
        try_parse_photodb(&data, &mut parsed).expect("BE parse (metadata)");
        assert_eq!(parsed.len(), 3, "BE metadata: 3 entries");
        for i in 0..3 {
            assert_eq!(
                parsed[i].metadata.mhod_strings[0], "MyAlbum",
                "BE entry {i}: album name",
            );
            assert_eq!(parsed[i].metadata.mhif_info_type, Some(42), "BE entry {i}: info type",);
            assert_eq!(parsed[i].data, entries[i].data, "BE entry {i}: pixel data preserved",);
        }
    }
}
