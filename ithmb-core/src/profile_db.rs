//! Profile database — loads profiles from embedded or external JSON.
//!
//! The built-in profile data is embedded at compile time via `include_str!`.
//! An external `profiles.json` can override entries at runtime.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::error::DecodeError;
use crate::profile::Profile;
use crate::profile_parser::parse_profiles_json;

/// An in-memory profile database keyed by format prefix.
#[derive(Debug, Clone)]
pub struct ProfileDb {
    profiles: HashMap<i32, Profile>,
}

impl ProfileDb {
    /// Load built-in profiles from the embedded `data/profiles.json`.
    ///
    /// This is the canonical source of the 53 active (+1 speculative) format
    /// profiles derived from the C# reference implementation.
    ///
    /// # Errors
    /// Returns `DecodeError::Profile` if the embedded JSON cannot be parsed.
    pub fn load_builtin() -> Result<Self, DecodeError> {
        let json = include_str!("../data/profiles.json");
        let profiles = parse_profiles_json(json)?;
        let map: HashMap<i32, Profile> = profiles.into_iter().map(|p| (p.prefix, p)).collect();
        Ok(Self { profiles: map })
    }

    /// Load an external `profiles.json` file and merge its entries,
    /// overriding any existing profiles by matching prefix.
    ///
    /// # Errors
    /// Returns `DecodeError::Profile` if the file cannot be read or parsed.
    pub fn load_external<P: AsRef<Path>>(&mut self, path: P) -> Result<(), DecodeError> {
        let data = fs::read_to_string(path.as_ref())
            .map_err(|e| DecodeError::Profile(format!("failed to read '{}': {e}", path.as_ref().display())))?;
        let profiles = parse_profiles_json(&data)?;
        for p in profiles {
            self.profiles.insert(p.prefix, p);
        }
        Ok(())
    }

    /// Look up a profile by its big-endian format prefix.
    #[must_use]
    pub fn get(&self, prefix: i32) -> Option<&Profile> {
        self.profiles.get(&prefix)
    }

    /// Return a reference to the entire profile map.
    #[must_use]
    pub fn all(&self) -> &HashMap<i32, Profile> {
        &self.profiles
    }

    /// Return the number of profiles in the database.
    #[must_use]
    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    /// Returns `true` when no profiles are loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
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
    fn load_builtin_has_54_profiles() {
        let db = ProfileDb::load_builtin().unwrap();
        assert_eq!(db.len(), 54);
    }

    #[test]
    fn get_1007_returns_correct_profile() {
        let db = ProfileDb::load_builtin().unwrap();
        let p = db.get(1007).expect("profile 1007 should exist");
        assert_eq!(p.prefix, 1007);
        assert_eq!(p.width, 480);
        assert_eq!(p.height, 864);
        assert_eq!(p.encoding, crate::profile::Encoding::Rgb565);
        assert_eq!(p.frame_byte_length, 829440);
        assert!(!p.is_padded);
        assert!(p.little_endian); // default
    }

    #[test]
    fn get_9999_returns_none() {
        let db = ProfileDb::load_builtin().unwrap();
        assert!(db.get(9999).is_none());
    }

    #[test]
    fn get_1061_has_use_mhni_dimensions() {
        let db = ProfileDb::load_builtin().unwrap();
        let p = db.get(1061).expect("profile 1061 should exist");
        assert!(p.use_mhni_dimensions);
    }

    #[test]
    fn get_3004_has_padded_slot() {
        let db = ProfileDb::load_builtin().unwrap();
        let p = db.get(3004).expect("profile 3004 should exist");
        assert!(p.is_padded);
        assert_eq!(p.slot_size, 8192);
        assert_eq!(p.encoding, crate::profile::Encoding::Rgb555);
    }

    #[test]
    fn all_returns_full_map() {
        let db = ProfileDb::load_builtin().unwrap();
        let all = db.all();
        assert!(all.contains_key(&1007));
        assert!(all.contains_key(&3011));
        assert_eq!(all.len(), 54);
    }
}
