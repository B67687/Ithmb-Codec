//! Post-processing: dimension swap, crop, and rotation.

use crate::config;
use crate::error::DecodedImage;
use crate::profile::Profile;

/// Applies dimension swap, crop, and rotation in that order.
pub(crate) fn apply_post_process(mut img: DecodedImage, profile: &Profile) -> DecodedImage {
    // 1. Swap display dimensions if the profile requests it.
    if profile.swaps_dimensions {
        std::mem::swap(&mut img.width, &mut img.height);
    }

    // 2. Crop to the visible region.
    img = apply_crop(img, profile);

    // 3. Rotate according to the profile's rotation field.
    apply_rotation(img, profile)
}

/// Applies dimension swap, crop, and rotation in that order, with runtime
/// overrides taking precedence over the profile's own fields.
///
/// For fields the caller does not override (`None`), the profile's value is used
/// -- an identity [`TransformConfig`](crate::config::TransformConfig) (default)
/// produces output identical to [`apply_post_process`].
pub(crate) fn apply_post_process_with_transform(
    mut img: DecodedImage,
    profile: &Profile,
    transform: &config::TransformConfig,
) -> DecodedImage {
    // 1. Swap display dimensions if the profile requests it.
    if profile.swaps_dimensions {
        std::mem::swap(&mut img.width, &mut img.height);
    }

    // 2. Crop -- runtime override wins over the profile's crop fields.
    img = match transform.crop() {
        Some(crop) => apply_crop_with(img, crop),
        None => apply_crop(img, profile),
    };

    // 3. Rotate -- runtime override wins over the profile's rotation field.
    let rotation = transform.rotation().unwrap_or(profile.rotation);
    apply_rotation_with(img, rotation)
}

/// Rotates by an explicit angle (degrees; 0/90/180/270, others no-op).
///
/// The `DecodedImage` is passed by value for pipeline symmetry with the other
/// post-processing steps (crop, swap) -- historic `rotate_90_cw`/`180`/`270_cw`
/// carried the same allow.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn apply_rotation_with(img: DecodedImage, rotation: i32) -> DecodedImage {
    let (data, width, height) = crate::pixel_utils::rotate_pixels(&img.data, img.width, img.height, rotation);
    DecodedImage { data, width, height }
}

/// Crops the image to the region specified by the profile.
///
/// When `crop_width` or `crop_height` is 0 the remaining span from the
/// corresponding offset is used. All values are clamped to the image bounds.
pub(crate) fn apply_crop(img: DecodedImage, profile: &Profile) -> DecodedImage {
    let needs_crop = profile.crop_x != 0 || profile.crop_y != 0 || profile.crop_width != 0 || profile.crop_height != 0;

    if !needs_crop {
        return img;
    }
    apply_crop_with(
        img,
        config::Crop {
            x: profile.crop_x,
            y: profile.crop_y,
            width: profile.crop_width,
            height: profile.crop_height,
        },
    )
}

/// Crops the image to the region specified by `crop` (0 width/height = remaining
/// span from the corresponding offset; all values clamped to the image bounds).
pub(crate) fn apply_crop_with(img: DecodedImage, crop: config::Crop) -> DecodedImage {
    #[allow(clippy::cast_sign_loss)]
    let cx = crop.x.max(0) as usize;
    #[allow(clippy::cast_sign_loss)]
    let cy = crop.y.max(0) as usize;
    let iw = img.width as usize;
    let ih = img.height as usize;

    #[allow(clippy::cast_sign_loss)]
    let cw = if crop.width > 0 {
        crop.width as usize
    } else {
        iw.saturating_sub(cx)
    };

    #[allow(clippy::cast_sign_loss)]
    let ch = if crop.height > 0 {
        crop.height as usize
    } else {
        ih.saturating_sub(cy)
    };

    // Clamp to image bounds.
    let cw = cw.min(iw.saturating_sub(cx));
    let ch = ch.min(ih.saturating_sub(cy));

    if cw == 0 || ch == 0 {
        return img;
    }

    let cap = cw.checked_mul(ch).and_then(|v| v.checked_mul(4)).unwrap_or(0);
    let mut cropped = Vec::with_capacity(cap);
    for y in cy..cy + ch {
        let row_start = (y * iw + cx) * 4;
        cropped.extend_from_slice(&img.data[row_start..row_start + cw * 4]);
    }

    #[allow(clippy::cast_possible_truncation)]
    DecodedImage {
        data: cropped,
        width: cw as u32,
        height: ch as u32,
    }
}

/// Applies the rotation specified by the profile.
///
/// Supports 0, 90, 180, and 270 clockwise rotation. Other values are
/// silently ignored.
pub(crate) fn apply_rotation(img: DecodedImage, profile: &Profile) -> DecodedImage {
    if profile.rotation % 360 == 0 {
        return img;
    }
    apply_rotation_with(img, profile.rotation)
}
