//! Hand-rolled JSON parser for the profiles.json schema.
//!
//! Parses a fixed-format JSON array of profile objects into `Vec<Profile>`.
//! Not a general-purpose JSON parser — understands only the field names and
//! value types used by the profile database.
//!
//! Split into sub-modules:
//! * `parser` - cursor-based JSON parser (Tokenizer + low-level helpers)
//! * `profile` - profile-specific parsing (array, object, field dispatch)

mod parser;
mod profile;

use crate::error::DecodeError;
use crate::profile::Profile;

/// Maximum number of profile objects accepted in a single `profiles.json`
/// document, matching the C# reference parser (`JsonParser.cs:28`).
const MAX_PROFILES: usize = 100;

/// Parse a JSON array of profile objects from the given string input.
///
/// # Errors
/// Returns `DecodeError::Profile` on invalid JSON, unknown encoding values,
/// or numeric parse failures.
pub fn parse_profiles_json(input: &str) -> Result<Vec<Profile>, DecodeError> {
    let mut p = parser::Parser {
        bytes: input.as_bytes(),
        pos: 0,
    };
    p.skip_ws();
    p.parse_array()
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
        assert_eq!(profiles[0].encoding, crate::profile::Encoding::Rgb565);
        assert_eq!(profiles[0].frame_byte_length, 829_440);
        assert!(!profiles[0].is_interlaced);

        assert_eq!(profiles[1].prefix, 1019);
        assert_eq!(profiles[1].encoding, crate::profile::Encoding::Yuv422);
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
        assert_eq!(fb[0], crate::profile::Encoding::Jpeg);
    }

    #[test]
    fn parse_all_54_profiles() {
        let json = include_str!("../../data/profiles.json");
        let profiles = parse_profiles_json(json).unwrap();
        assert_eq!(profiles.len(), 54);
        let p1007 = profiles.iter().find(|p| p.prefix == 1007).unwrap();
        assert_eq!(p1007.width, 480);
        assert_eq!(p1007.height, 864);
        assert_eq!(p1007.encoding, crate::profile::Encoding::Rgb565);

        let p3004 = profiles.iter().find(|p| p.prefix == 3004).unwrap();
        assert!(p3004.is_padded);
        assert_eq!(p3004.slot_size, 8192);
        assert_eq!(p3004.encoding, crate::profile::Encoding::Rgb555);

        let p1042 = profiles.iter().find(|p| p.prefix == 1042).unwrap();
        assert_eq!(p1042.width, 320);

        let p1081 = profiles.iter().find(|p| p.prefix == 1081).unwrap();
        let fb = p1081.fallback_encodings.as_ref().unwrap();
        assert_eq!(fb.as_slice(), &[crate::profile::Encoding::Jpeg]);

        let p2002 = profiles.iter().find(|p| p.prefix == 2002).unwrap();
        assert!(!p2002.little_endian);
    }

    #[test]
    fn parse_error_on_bad_json() {
        let result = parse_profiles_json("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn skip_value_does_not_hang_on_unclosed_nested_brackets() {
        for input in ["[{[[", "[{ [\r", "[[", "[[[", "[{\r[", "[ \r[[", "["] {
            assert!(
                parse_profiles_json(input).is_err(),
                "expected a parse error for {input:?} instead of a hang",
            );
        }
    }

    #[test]
    fn rejects_more_than_100_profile_objects() {
        let json = format!(
            "[{}]",
            (0..101)
                .map(|i| format!(r#"{{"prefix":{i}}}"#))
                .collect::<Vec<_>>()
                .join(","),
        );
        let err = parse_profiles_json(&json).unwrap_err();
        assert!(
            err.to_string().contains("100"),
            "error should mention the 100-object cap, got: {err}",
        );
    }

    #[test]
    fn accepts_exactly_100_profile_objects() {
        let json = format!(
            "[{}]",
            (0..100)
                .map(|i| format!(r#"{{"prefix":{i}}}"#))
                .collect::<Vec<_>>()
                .join(","),
        );
        let profiles = parse_profiles_json(&json).unwrap();
        assert_eq!(profiles.len(), 100);
    }
}
