use std::path::Path;

use anyhow::{Result, bail};
use ithmb_core::Profile;
use ithmb_core::profile_db::ProfileDb;

/// Parse the format id from an F-prefix filename such as `F1061_1.ithmb`.
pub fn f_filename_format_id(path: &Path) -> Option<i32> {
    let name = path.file_name()?.to_str()?;
    let after_f = name.strip_prefix('F')?;
    let digits: String = after_f.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

/// Frame layout of an `.ithmb` file.
#[derive(Debug)]
pub struct FrameLayout {
    /// Number of frames in the file.
    pub count: usize,
    /// Byte offset of the first frame payload within the file buffer.
    pub data_offset: usize,
    /// Size in bytes of one frame payload.
    pub frame_size: usize,
    /// 4-byte big-endian format-id prefix prepended to extracted frames.
    pub prefix_bytes: Option<[u8; 4]>,
}

impl FrameLayout {
    /// Payload bytes of frame `index` within the file buffer.
    pub fn frame_bytes<'a>(&self, data: &'a [u8], index: usize) -> &'a [u8] {
        let start = self.data_offset + index * self.frame_size;
        &data[start..start + self.frame_size]
    }

    /// Serialized bytes for extracted frame `index`.
    pub fn extracted_bytes(&self, data: &[u8], index: usize) -> Vec<u8> {
        let payload = self.frame_bytes(data, index);
        match self.prefix_bytes {
            Some(prefix) => {
                let mut out = Vec::with_capacity(4 + payload.len());
                out.extend_from_slice(&prefix);
                out.extend_from_slice(payload);
                out
            }
            None => payload.to_vec(),
        }
    }
}

/// Resolve how frames are laid out in `data` for the given input path.
pub fn resolve_frame_layout(data: &[u8], input: &Path, db: &ProfileDb) -> Result<FrameLayout> {
    if data.len() < 4 {
        bail!("file too short: expected at least 4 bytes");
    }

    let prefix = i32::from_be_bytes([data[0], data[1], data[2], data[3]]);

    if data[0] == 0xFF && data[1] == 0xD8 {
        return Ok(FrameLayout {
            count: 1,
            data_offset: 0,
            frame_size: data.len(),
            prefix_bytes: None,
        });
    }

    if let Some(profile) = db.get(prefix) {
        return raw_layout(profile, data, 4);
    }

    if let Some(format_id) = f_filename_format_id(input) {
        if let Some(profile) = db.get(format_id) {
            return raw_layout(profile, data, 0);
        }
        bail!("unknown format prefix {format_id} (from F-filename)");
    }

    bail!(
        "cannot determine format of '{}': unknown prefix {prefix}",
        input.display()
    );
}

/// Frame layout for a raw (non-JPEG) file.
fn raw_layout(profile: &Profile, data: &[u8], data_offset: usize) -> Result<FrameLayout> {
    #[allow(clippy::cast_sign_loss)]
    let declared_frame_size = profile.frame_size() as usize;
    if declared_frame_size == 0 {
        bail!("profile {} has no frame size", profile.prefix);
    }
    #[allow(clippy::cast_sign_loss)]
    let prefix_bytes = (profile.prefix as u32).to_be_bytes();

    let payload_len = data.len() - data_offset;
    if payload_len == 0 {
        bail!("file too small to hold a full frame (0 payload bytes)");
    }
    let frame_size = payload_len.min(declared_frame_size);
    let count = payload_len / frame_size;

    Ok(FrameLayout {
        count,
        data_offset,
        frame_size,
        prefix_bytes: Some(prefix_bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ithmb_core::profile_db::ProfileDb;

    fn builtin_db() -> ProfileDb {
        ProfileDb::load_builtin().expect("built-in profile database loads")
    }

    #[allow(clippy::cast_sign_loss)]
    fn prefixed_buffer(prefix: i32, frame_size: usize, frames: usize) -> Vec<u8> {
        let mut buf = (prefix as u32).to_be_bytes().to_vec();
        buf.resize(4 + frame_size * frames, 0);
        buf
    }

    fn f_buffer(frame_size: usize, frames: usize) -> Vec<u8> {
        vec![0; frame_size * frames]
    }

    #[test]
    fn f_filename_format_id_parses_f_names() {
        assert_eq!(f_filename_format_id(Path::new("F1061_1.ithmb")), Some(1061));
        assert_eq!(f_filename_format_id(Path::new("F1019_1.ithmb")), Some(1019));
        assert_eq!(f_filename_format_id(Path::new("F1055_1.ithmb")), Some(1055));
    }

    #[test]
    fn f_filename_format_id_rejects_other_names() {
        assert_eq!(f_filename_format_id(Path::new("sample.ithmb")), None);
        assert_eq!(f_filename_format_id(Path::new("F.ithmb")), None);
        assert_eq!(f_filename_format_id(Path::new("Fabc_1.ithmb")), None);
    }

    #[test]
    fn prefixed_multi_frame_layout() {
        let data = prefixed_buffer(1061, 6160, 10);
        let layout = resolve_frame_layout(&data, Path::new("F1061_1.ithmb"), &builtin_db()).unwrap();
        assert_eq!(layout.count, 10);
        assert_eq!(layout.data_offset, 4);
        assert_eq!(layout.frame_size, 6160);
        assert_eq!(layout.prefix_bytes, Some(1061_u32.to_be_bytes()));
    }

    #[test]
    fn prefixless_f_file_multi_frame_layout() {
        let data = f_buffer(6160, 10);
        let layout = resolve_frame_layout(&data, Path::new("F1061_1.ithmb"), &builtin_db()).unwrap();
        assert_eq!(layout.count, 10);
        assert_eq!(layout.data_offset, 0);
        assert_eq!(layout.frame_size, 6160);
        assert_eq!(layout.prefix_bytes, Some(1061_u32.to_be_bytes()));
    }

    #[test]
    fn single_frame_layouts() {
        let db = builtin_db();
        let prefixed =
            resolve_frame_layout(&prefixed_buffer(1024, 153_600, 1), Path::new("sample.ithmb"), &db).unwrap();
        assert_eq!(prefixed.count, 1);

        let f_file = resolve_frame_layout(&f_buffer(6160, 1), Path::new("F1061_1.ithmb"), &db).unwrap();
        assert_eq!(f_file.count, 1);
    }

    #[test]
    fn jpeg_stream_is_single_frame() {
        let mut data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        data.extend_from_slice(&[0u8; 64]);
        let layout = resolve_frame_layout(&data, Path::new("T1007.ithmb"), &builtin_db()).unwrap();
        assert_eq!(layout.count, 1);
        assert_eq!(layout.data_offset, 0);
        assert_eq!(layout.frame_size, data.len());
        assert_eq!(layout.prefix_bytes, None);
    }

    #[test]
    fn extracted_bytes_prepend_prefix() {
        let data = f_buffer(6160, 2);
        let layout = resolve_frame_layout(&data, Path::new("F1061_1.ithmb"), &builtin_db()).unwrap();
        let first = layout.extracted_bytes(&data, 0);
        let mut expected = 1061_u32.to_be_bytes().to_vec();
        expected.resize(4 + 6160, 0);
        assert_eq!(first, expected);
        assert_eq!(first.len(), 4 + 6160);
    }

    #[test]
    fn jpeg_extracted_bytes_are_whole_file() {
        let mut data = vec![0xAA; 96];
        data[0] = 0xFF;
        data[1] = 0xD8;
        let layout = resolve_frame_layout(&data, Path::new("T1007.ithmb"), &builtin_db()).unwrap();
        assert_eq!(layout.extracted_bytes(&data, 0), data);
    }

    #[test]
    fn unknown_format_errors() {
        let data = vec![0x12, 0x34, 0x56, 0x78, 0x00, 0x01, 0x02, 0x03];
        let err = resolve_frame_layout(&data, Path::new("mystery.ithmb"), &builtin_db())
            .expect_err("unknown prefix must fail");
        assert!(err.to_string().contains("cannot determine format"));
    }

    #[test]
    fn file_too_small_errors() {
        let db = builtin_db();
        let err = resolve_frame_layout(&[0u8, 0, 4, 0x25], Path::new("F1061_1.ithmb"), &db)
            .expect_err("empty payload must fail");
        assert!(err.to_string().contains("too small"));

        let err =
            resolve_frame_layout(&[0u8, 0, 0], Path::new("x.ithmb"), &db).expect_err("buffer under 4 bytes must fail");
        assert!(err.to_string().contains("too short"));
    }

    #[test]
    fn payload_smaller_than_frame_size_is_single_frame() {
        let mut buf = vec![0u8; 518_404];
        buf[..4].copy_from_slice(&1067_u32.to_be_bytes());
        let layout = resolve_frame_layout(&buf, Path::new("ycbcr420.ithmb"), &builtin_db()).unwrap();
        assert_eq!(layout.count, 1);
        assert_eq!(layout.frame_size, 518_400);
        assert_eq!(layout.extracted_bytes(&buf, 0), buf);
    }
}
