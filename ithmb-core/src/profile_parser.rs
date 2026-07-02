// allow: SIZE_OK — hand-rolled JSON parser is an indivisible state machine

//! Hand-rolled JSON parser for the profiles.json schema.
//!
//! Parses a fixed-format JSON array of profile objects into `Vec<Profile>`.
//! Not a general-purpose JSON parser — understands only the field names and
//! value types used by the profile database.

use crate::error::DecodeError;
use crate::profile::{Encoding, Profile};

/// Parse a JSON array of profile objects from the given string input.
///
/// # Errors
/// Returns `DecodeError::Profile` on invalid JSON, unknown encoding values,
/// or numeric parse failures.
pub fn parse_profiles_json(input: &str) -> Result<Vec<Profile>, DecodeError> {
    let mut p = Parser {
        bytes: input.as_bytes(),
        pos: 0,
    };
    p.skip_ws();
    p.parse_array()
}

// ---------------------------------------------------------------------------
// Internal cursor-based parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    // -- low-level helpers --

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn advance(&mut self) -> Result<u8, DecodeError> {
        let b = self
            .bytes
            .get(self.pos)
            .copied()
            .ok_or_else(|| DecodeError::Profile("unexpected end of input".into()))?;
        self.pos += 1;
        Ok(b)
    }

    fn expect(&mut self, want: u8) -> Result<(), DecodeError> {
        let got = self.advance()?;
        if got != want {
            return Err(DecodeError::Profile(format!(
                "expected '{}' (0x{:02X}), got '{}' (0x{:02X}) at offset {}",
                want as char,
                want,
                got as char,
                got,
                self.pos - 1,
            )));
        }
        Ok(())
    }

    fn skip_ws(&mut self) {
        while let Some(b) = self.peek() {
            if b != b' ' && b != b'\t' && b != b'\n' && b != b'\r' {
                break;
            }
            self.pos += 1;
        }
    }

    // -- value parsers --

    fn parse_string(&mut self) -> Result<String, DecodeError> {
        self.expect(b'"')?;
        let mut s = String::new();
        loop {
            let b = self.advance()?;
            match b {
                b'"' => return Ok(s),
                b'\\' => {
                    let esc = self.advance()?;
                    match esc {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'/' => s.push('/'),
                        b'b' => s.push('\u{0008}'),
                        b'f' => s.push('\u{000C}'),
                        b'n' => s.push('\n'),
                        b'r' => s.push('\r'),
                        b't' => s.push('\t'),
                        b'u' => {
                            let code = self.parse_hex4()?;
                            let c = char::from_u32(code).ok_or_else(|| {
                                DecodeError::Profile(format!("invalid unicode escape: \\u{code:04X}"))
                            })?;
                            s.push(c);
                        }
                        _ => {
                            return Err(DecodeError::Profile(format!(
                                "invalid escape sequence: \\{}",
                                esc as char
                            )));
                        }
                    }
                }
                // JSON allows most characters inside strings, including UTF-8
                0x20..=0x7E | 0x80..=0xFF => s.push(b as char),
                _ => {
                    return Err(DecodeError::Profile(format!("invalid character in string: 0x{b:02X}")));
                }
            }
        }
    }

    fn parse_hex4(&mut self) -> Result<u32, DecodeError> {
        let mut val: u32 = 0;
        for _ in 0..4 {
            let b = self.advance()?;
            val <<= 4;
            val += match b {
                b'0'..=b'9' => u32::from(b - b'0'),
                b'a'..=b'f' => u32::from(b - b'a' + 10),
                b'A'..=b'F' => u32::from(b - b'A' + 10),
                _ => {
                    return Err(DecodeError::Profile(format!("invalid hex digit: '{}'", b as char)));
                }
            };
        }
        Ok(val)
    }

    fn parse_number_i32(&mut self) -> Result<i32, DecodeError> {
        self.skip_ws();
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        if self.peek().is_none_or(|b| !b.is_ascii_digit()) {
            return Err(DecodeError::Profile(format!("expected number at offset {start}")));
        }
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.pos += 1;
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|e| DecodeError::Profile(format!("invalid number encoding: {e}")))?;
        s.parse::<i32>()
            .map_err(|e| DecodeError::Profile(format!("invalid number '{s}': {e}")))
    }

    fn parse_bool(&mut self) -> Result<bool, DecodeError> {
        self.skip_ws();
        if self.bytes[self.pos..].starts_with(b"true") {
            self.pos += 4;
            Ok(true)
        } else if self.bytes[self.pos..].starts_with(b"false") {
            self.pos += 5;
            Ok(false)
        } else {
            Err(DecodeError::Profile(format!("expected bool at offset {}", self.pos)))
        }
    }

    fn skip_value(&mut self) -> Result<(), DecodeError> {
        self.skip_ws();
        match self.peek() {
            Some(b'"') => {
                self.parse_string()?;
            }
            Some(b't' | b'f') => {
                self.parse_bool()?;
            }
            Some(b'n') => {
                self.parse_null()?;
            }
            Some(b'[') => {
                self.skip_array()?;
            }
            Some(b'{') => {
                self.skip_object()?;
            }
            Some(b'-' | b'0'..=b'9') => {
                self.parse_number_i32()?;
            }
            Some(c) => {
                return Err(DecodeError::Profile(format!(
                    "unexpected character '{c}' at offset {}",
                    self.pos
                )));
            }
            None => {
                return Err(DecodeError::Profile("unexpected end of input".into()));
            }
        }
        Ok(())
    }

    fn parse_null(&mut self) -> Result<(), DecodeError> {
        self.skip_ws();
        if self.bytes[self.pos..].starts_with(b"null") {
            self.pos += 4;
            Ok(())
        } else {
            Err(DecodeError::Profile(format!("expected null at offset {}", self.pos)))
        }
    }

    fn skip_array(&mut self) -> Result<(), DecodeError> {
        self.expect(b'[')?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(());
            }
            if self.peek() != Some(b'[') {
                self.skip_value()?;
            }
            self.skip_ws();
            if self.peek() == Some(b',') {
                self.pos += 1;
            }
        }
    }

    fn skip_object(&mut self) -> Result<(), DecodeError> {
        self.expect(b'{')?;
        loop {
            self.skip_ws();
            if self.peek() == Some(b'}') {
                self.pos += 1;
                return Ok(());
            }
            self.parse_string()?;
            self.skip_ws();
            self.expect(b':')?;
            self.skip_value()?;
            self.skip_ws();
            if self.peek() == Some(b',') {
                self.pos += 1;
            }
        }
    }

    // -- profile-specific parsers --

    fn parse_array(&mut self) -> Result<Vec<Profile>, DecodeError> {
        self.expect(b'[')?;
        let mut profiles: Vec<Profile> = Vec::new();
        loop {
            self.skip_ws();
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(profiles);
            }
            if !profiles.is_empty() {
                self.expect(b',')?;
                self.skip_ws();
            }
            profiles.push(self.parse_object()?);
        }
    }

    fn parse_object(&mut self) -> Result<Profile, DecodeError> {
        self.expect(b'{')?;
        let mut profile = Profile::default();
        loop {
            self.skip_ws();
            if self.peek() == Some(b'}') {
                self.pos += 1;
                return Ok(profile);
            }
            if self.peek() != Some(b'"') {
                // Keys that are not objects — skip the entry
                self.skip_value()?;
                continue;
            }
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(b':')?;
            self.set_field(&key, &mut profile)?;
            self.skip_ws();
            if self.peek() == Some(b',') {
                self.pos += 1;
            }
        }
    }

    fn set_field(&mut self, key: &str, p: &mut Profile) -> Result<(), DecodeError> {
        match key {
            "prefix" => p.prefix = self.parse_number_i32()?,
            "width" => p.width = self.parse_number_i32()?,
            "height" => p.height = self.parse_number_i32()?,
            "encoding" => p.encoding = self.parse_encoding()?,
            "frame_byte_length" => p.frame_byte_length = self.parse_number_i32()?,
            "swaps_dimensions" => p.swaps_dimensions = self.parse_bool()?,
            "little_endian" => p.little_endian = self.parse_bool()?,
            "is_padded" => p.is_padded = self.parse_bool()?,
            "is_interlaced" => p.is_interlaced = self.parse_bool()?,
            "clcl_chroma" => p.clcl_chroma = self.parse_bool()?,
            "swap_chroma_planes" => p.swap_chroma_planes = self.parse_bool()?,
            "cl_chroma" => p.cl_chroma = self.parse_bool()?,
            "swap_rgb_channels" => p.swap_rgb_channels = self.parse_bool()?,
            "rotation" => p.rotation = self.parse_number_i32()?,
            "crop_x" => p.crop_x = self.parse_number_i32()?,
            "crop_y" => p.crop_y = self.parse_number_i32()?,
            "crop_width" => p.crop_width = self.parse_number_i32()?,
            "crop_height" => p.crop_height = self.parse_number_i32()?,
            "slot_size" => p.slot_size = self.parse_number_i32()?,
            "use_mhni_dimensions" => p.use_mhni_dimensions = self.parse_bool()?,
            "fallback_encodings" => {
                if self.peek() == Some(b'n') {
                    self.parse_null()?;
                } else {
                    p.fallback_encodings = Some(self.parse_encoding_array()?);
                }
            }
            // Unknown field – skip its value to stay compatible
            // with extended schemas.
            _ => self.skip_value()?,
        }
        Ok(())
    }

    fn parse_encoding(&mut self) -> Result<Encoding, DecodeError> {
        let s = self.parse_string()?;
        match s.to_lowercase().as_str() {
            "rgb565" => Ok(Encoding::Rgb565),
            "rgb555" => Ok(Encoding::Rgb555),
            "reorderedrgb555" => Ok(Encoding::ReorderedRgb555),
            "yuv422" => Ok(Encoding::Yuv422),
            "ycbcr420" => Ok(Encoding::Ycbcr420),
            "jpeg" => Ok(Encoding::Jpeg),
            _ => Err(DecodeError::Profile(format!("unknown encoding: '{s}'"))),
        }
    }

    fn parse_encoding_array(&mut self) -> Result<Vec<Encoding>, DecodeError> {
        self.expect(b'[')?;
        let mut encodings: Vec<Encoding> = Vec::new();
        loop {
            self.skip_ws();
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(encodings);
            }
            if !encodings.is_empty() {
                self.expect(b',')?;
                self.skip_ws();
            }
            encodings.push(self.parse_encoding()?);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_profiles_json() {
        let json = r#"[
            {"prefix":1007,"width":480,"height":864,"encoding":"Rgb565","frame_byte_length":829440},
            {"prefix":1019,"width":720,"height":480,"encoding":"Yuv422","frame_byte_length":691200,"is_interlaced":true}
        ]"#;
        let profiles = parse_profiles_json(json).unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].prefix, 1007);
        assert_eq!(profiles[0].width, 480);
        assert_eq!(profiles[0].height, 864);
        assert_eq!(profiles[0].encoding, Encoding::Rgb565);
        assert_eq!(profiles[0].frame_byte_length, 829440);
        assert!(!profiles[0].is_interlaced);

        assert_eq!(profiles[1].prefix, 1019);
        assert_eq!(profiles[1].encoding, Encoding::Yuv422);
        assert!(profiles[1].is_interlaced);
    }

    #[test]
    fn parse_with_fallback_encodings() {
        let json = r#"[
            {"prefix":1081,"width":640,"height":480,"encoding":"Rgb565","frame_byte_length":614400,"fallback_encodings":["Jpeg"]}
        ]"#;
        let profiles = parse_profiles_json(json).unwrap();
        assert_eq!(profiles.len(), 1);
        let fb = profiles[0].fallback_encodings.as_ref().unwrap();
        assert_eq!(fb.len(), 1);
        assert_eq!(fb[0], Encoding::Jpeg);
    }

    #[test]
    fn parse_all_54_profiles() {
        let json = include_str!("../data/profiles.json");
        let profiles = parse_profiles_json(json).unwrap();
        assert_eq!(profiles.len(), 54);
        // Spot-check a few entries
        let p1007 = profiles.iter().find(|p| p.prefix == 1007).unwrap();
        assert_eq!(p1007.width, 480);
        assert_eq!(p1007.height, 864);
        assert_eq!(p1007.encoding, Encoding::Rgb565);

        let p3004 = profiles.iter().find(|p| p.prefix == 3004).unwrap();
        assert!(p3004.is_padded);
        assert_eq!(p3004.slot_size, 8192);
        assert_eq!(p3004.encoding, Encoding::Rgb555);

        let p1042 = profiles.iter().find(|p| p.prefix == 1042).unwrap();
        assert_eq!(p1042.width, 320);

        // Fallback encodings
        let p1081 = profiles.iter().find(|p| p.prefix == 1081).unwrap();
        let fb = p1081.fallback_encodings.as_ref().unwrap();
        assert_eq!(fb.as_slice(), &[Encoding::Jpeg]);

        // Big-endian profiles
        let p2002 = profiles.iter().find(|p| p.prefix == 2002).unwrap();
        assert!(!p2002.little_endian);
    }

    #[test]
    fn parse_error_on_bad_json() {
        let result = parse_profiles_json("not json at all");
        assert!(result.is_err());
    }
}
