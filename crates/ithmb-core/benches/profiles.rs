// SPDX-License-Identifier: MIT
//! Benchmarks for all 54 built-in profiles — measures decode throughput for
//! every profile at its native dimensions using `decode_with_profile`.
//!
//! Each profile generates synthetic BGRA pixel data at its exact dimensions,
//! encodes via `build_ithmb_file`, and benchmarks decode throughput. Profiles
//! are grouped by encoding type for comparison.

#![allow(
    clippy::pedantic,
    clippy::unwrap_used,
    elided_lifetimes_in_paths,
    unused_crate_dependencies
)]

mod util;

use divan::counter::BytesCount;
use ithmb_core::enc::build_ithmb_file;
use ithmb_core::pipeline::decode_with_profile;
use ithmb_core::profile::{Encoding, Profile};
use ithmb_core::profile_db::ProfileDb;
use std::fmt;
use std::sync::atomic::AtomicBool;
use util::bgra_checkerboard;

// ---------------------------------------------------------------------------
// Benchmark argument — one per profile, sorted by prefix
// ---------------------------------------------------------------------------

/// A single profile benchmark argument carrying pre-encoded data and metadata.
struct ProfileBenchArg {
    /// Human-readable label: `p{PREFIX}_{ENCODING}_{W}x{H}`
    name: String,
    /// Full .ithmb file bytes (4-byte prefix + encoded pixel data + padding)
    data: Vec<u8>,
    /// Profile describing how to decode `data`
    profile: Profile,
    /// Number of BGRA output bytes (w × h × 4)
    output_bytes: u64,
}

impl fmt::Display for ProfileBenchArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

// ---------------------------------------------------------------------------
// Build all profile arguments once
// ---------------------------------------------------------------------------

/// Return a short label for the profile's effective encoding.
#[must_use]
fn encoding_label(p: &Profile) -> &'static str {
    if p.clcl_chroma {
        "CLCL"
    } else if p.cl_chroma {
        "CL"
    } else {
        match p.encoding {
            Encoding::Rgb565 => "RGB565",
            Encoding::Rgb555 => "RGB555",
            Encoding::ReorderedRgb555 => "ReorderedRGB555",
            Encoding::Yuv422 => "UYVY",
            Encoding::Ycbcr420 => "YCbCr420",
            Encoding::Jpeg => "JPEG",
            _ => unreachable!(),
        }
    }
}

/// Build a `ProfileBenchArg` for every non-JPEG built-in profile.
#[must_use]
fn build_all_args() -> Vec<ProfileBenchArg> {
    let db = ProfileDb::load_builtin().expect("built-in profiles must load");
    let mut profiles: Vec<(i32, &Profile)> = db.all().iter().map(|(&k, v)| (k, v)).collect();
    profiles.sort_by_key(|&(k, _)| k);

    profiles
        .into_iter()
        .map(|(_prefix, p)| {
            let w = p.width as usize;
            let h = p.height as usize;
            let bgra = bgra_checkerboard(w, h);
            let data = build_ithmb_file(&bgra, p.width, p.height, p);
            let output_bytes = (p.width as u64 * p.height as u64) * 4;
            let name = format!("p{}_{}_{}x{}", p.prefix, encoding_label(p), p.width, p.height);
            ProfileBenchArg {
                name,
                data,
                profile: p.clone(),
                output_bytes,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Macro: create an args-function + bench-function pair per encoding group
// ---------------------------------------------------------------------------

macro_rules! profile_bench_group {
    ($args_fn:ident, $bench_fn:ident, $filter:expr) => {
        #[must_use]
        fn $args_fn() -> &'static [ProfileBenchArg] {
            use std::sync::OnceLock;
            static ARGS: OnceLock<Vec<ProfileBenchArg>> = OnceLock::new();
            let args = ARGS.get_or_init(|| {
                let mut filtered: Vec<ProfileBenchArg> = build_all_args().into_iter().filter($filter).collect();
                filtered.sort_by(|a, b| a.name.cmp(&b.name));
                filtered
            });
            args.as_slice()
        }

        #[divan::bench(args = $args_fn())]
        #[allow(non_snake_case)]
        fn $bench_fn(bencher: divan::Bencher, arg: &ProfileBenchArg) {
            let canceled = AtomicBool::new(false);
            bencher
                .counter(BytesCount::new(arg.output_bytes))
                .with_inputs(|| (arg.data.as_slice(), &arg.profile, &canceled))
                .bench_refs(|(data, profile, canceled)| {
                    let _ = divan::black_box(decode_with_profile(data, profile, canceled));
                });
        }
    };
}

// ---------------------------------------------------------------------------
// Profile groups — one bench function per encoding type
// ---------------------------------------------------------------------------

profile_bench_group!(rgb565_args, decode_rgb565_profiles, |a: &ProfileBenchArg| {
    !a.profile.clcl_chroma && !a.profile.cl_chroma && a.profile.encoding == Encoding::Rgb565
});

profile_bench_group!(rgb555_args, decode_rgb555_profiles, |a: &ProfileBenchArg| {
    !a.profile.clcl_chroma && !a.profile.cl_chroma && a.profile.encoding == Encoding::Rgb555
});

profile_bench_group!(
    reordered_rgb555_args,
    decode_reordered_rgb555_profiles,
    |a: &ProfileBenchArg| {
        !a.profile.clcl_chroma && !a.profile.cl_chroma && a.profile.encoding == Encoding::ReorderedRgb555
    }
);

profile_bench_group!(uyvy_args, decode_uyvy_profiles, |a: &ProfileBenchArg| {
    !a.profile.clcl_chroma && !a.profile.cl_chroma && a.profile.encoding == Encoding::Yuv422
});

profile_bench_group!(ycbcr420_args, decode_ycbcr420_profiles, |a: &ProfileBenchArg| {
    !a.profile.clcl_chroma && !a.profile.cl_chroma && a.profile.encoding == Encoding::Ycbcr420
});

// CLCL and CL groups are included for external profile overrides; no built-in
// profile currently uses these chroma flags.

profile_bench_group!(clcl_args, decode_clcl_profiles, |a: &ProfileBenchArg| a
    .profile
    .clcl_chroma);

profile_bench_group!(cl_args, decode_cl_profiles, |a: &ProfileBenchArg| a.profile.cl_chroma);

fn main() {
    divan::main();
}
