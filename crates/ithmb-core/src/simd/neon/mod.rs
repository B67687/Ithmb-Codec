//! AArch64 NEON SIMD implementations for pixel conversions.
//! Compiled when target is `aarch64`.

mod cl;
mod clcl;
mod rgb;
mod yuv;

pub(crate) use self::cl::{cl_quad_to_bgra_neon, cl_row_to_bgra_neon};
pub(crate) use self::clcl::clcl_row_to_bgra_neon;
pub(crate) use self::rgb::{fill_gray_row_neon, rgb555_row_to_bgra_neon, rgb565_row_to_bgra_neon};
pub(crate) use self::yuv::{uyvy_double_quad_to_bgra_neon, uyvy_quad_to_bgra_neon, yuv420_row_pair_to_bgra_neon};
