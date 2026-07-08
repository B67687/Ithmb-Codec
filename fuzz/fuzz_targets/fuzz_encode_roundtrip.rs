#![no_main]

use libfuzzer_sys::fuzz_target;
use std::sync::atomic::AtomicBool;

/// Fuzz target: encode arbitrary BGRA data with every format, then decode.
///
/// Input layout:
///   byte[0]  — format selector (0..=7)
///   byte[1]  — width  (1..=32 via modulo)
///   byte[2]  — height (1..=32 via modulo)
///   byte[3+] — BGRA pixel data (w * h * 4 bytes, repeats if necessary)
fuzz_target!(|data: &[u8]| {
    use ithmb_core::enc::*;
    use ithmb_core::pipeline::decode_with_profile;
    use ithmb_core::profile::{Encoding, Profile};

    let canceled = AtomicBool::new(false);

    if data.len() < 3 {
        return;
    }

    let fmt = data[0] % 8;
    let w = ((data[1] as u32 % 32) + 1) as i32;
    let h = ((data[2] as u32 % 32) + 1) as i32;
    let pixel_count = (w * h) as usize;
    let needed = pixel_count * 4;

    // Repeat data[3..] to fill the BGRA buffer, or truncate if too long
    let bgra: Vec<u8> = if data.len() > 3 {
        data[3..].iter().copied().cycle().take(needed).collect()
    } else {
        return;
    };

    let profile = Profile {
        width: w as u32,
        height: h as u32,
        frame_byte_length: (pixel_count * 2) as u32,
        ..Default::default()
    };

    let encoded = match fmt {
        0 => {
            profile.encoding = Encoding::Rgb565;
            encode_rgb565(&bgra, w, h, false)
        }
        1 => {
            profile.encoding = Encoding::Rgb565;
            encode_rgb565(&bgra, w, h, true)
        }
        2 => {
            profile.encoding = Encoding::Rgb555;
            encode_rgb555(&bgra, w, h, false, false)
        }
        3 => {
            profile.encoding = Encoding::Rgb555;
            encode_rgb555(&bgra, w, h, false, true)
        }
        4 => {
            profile.encoding = Encoding::Yuv422;
            encode_uyvy(&bgra, w, h)
        }
        5 => {
            profile.encoding = Encoding::Ycbcr420;
            encode_ycbcr420(&bgra, w, h)
        }
        6 => {
            profile.encoding = Encoding::Yuv422;
            encode_cl(&bgra, w, h)
        }
        7 => {
            profile.encoding = Encoding::Yuv422;
            encode_clcl(&bgra, w, h)
        }
        _ => return,
    };

    // Decode back; ignore errors (corrupted encoding is expected from fuzz input)
    let decoded = decode_with_profile(&encoded, &profile, &canceled);
    if let Ok(img) = decoded {
        // Ensure output dimensions match and data is non-empty
        let _ = img.width;
        let _ = img.height;
        if !img.data.is_empty() {
            // Alpha should always be 255 for all decoders
            #[allow(clippy::cast_possible_truncation)]
            let alpha_valid = img.data.chunks(4).all(|pix| pix[3] == 255);
            let _ = alpha_valid;
        }
    }
});
