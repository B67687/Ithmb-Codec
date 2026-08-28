use crate::error::DecodeError;
use crate::profile::{Encoding, Profile};

use super::MAX_PROFILES;
use super::parser::Parser;

impl Parser<'_> {
    // -- profile-specific parsers --

    pub fn parse_array(&mut self) -> Result<Vec<Profile>, DecodeError> {
        self.expect(b'[')?;
        let mut profiles: Vec<Profile> = Vec::new();
        loop {
            self.skip_ws();
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(profiles);
            }
            if profiles.len() >= MAX_PROFILES {
                return Err(DecodeError::Profile(format!(
                    "profile array exceeds the maximum of {MAX_PROFILES} objects"
                )));
            }
            if !profiles.is_empty() {
                self.expect(b',')?;
                self.skip_ws();
            }
            profiles.push(self.parse_object()?);
        }
    }

    pub fn parse_object(&mut self) -> Result<Profile, DecodeError> {
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

    pub fn set_field(&mut self, key: &str, p: &mut Profile) -> Result<(), DecodeError> {
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

    pub fn parse_encoding(&mut self) -> Result<Encoding, DecodeError> {
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

    pub fn parse_encoding_array(&mut self) -> Result<Vec<Encoding>, DecodeError> {
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
