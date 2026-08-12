//! Profile database — loads profiles from embedded or external JSON.
//!
//! The built-in profile data is embedded at compile time via `include_str!`.
//! An external `profiles.json` can override entries at runtime.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::error::DecodeError;
use crate::profile::{Encoding, Profile};
use crate::profile_parser::parse_profiles_json;

/// Built-in profile prefixes deliberately kept OUT of the active set.
///
/// Mirrors the C# reference (`IthmbCodecPlugin.ProfilesJson.cs`) where these
/// entries are kept commented out — profile 1044 is disabled because writing it
/// to an iPod Classic corrupts cover art (iOpenPod #81).
const DISABLED_PREFIXES: &[i32] = &[1044];

/// An in-memory profile database keyed by format prefix.
#[derive(Debug, Clone)]
pub struct ProfileDb {
    profiles: HashMap<i32, Profile>,
    /// Per-prefix device-override alternates (Nano 7G cover-art formats),
    /// selected by data length before falling back to the main profile.
    alternates: HashMap<i32, Vec<Profile>>,
}

impl ProfileDb {
    /// Load built-in profiles from the embedded `data/profiles.json`.
    ///
    /// This is the canonical source of the 53 active format profiles derived
    /// from the C# reference implementation. The 54th JSON entry (prefix 1044)
    /// is parsed but filtered out via the `DISABLED_PREFIXES` constant — it
    /// mirrors the C# reference where 1044 stays commented out.
    ///
    /// # Errors
    /// Returns `DecodeError::Profile` if the embedded JSON cannot be parsed.
    pub fn load_builtin() -> Result<Self, DecodeError> {
        let json = include_str!("../data/profiles.json");
        let profiles = parse_profiles_json(json)?;
        let mut map: HashMap<i32, Profile> = HashMap::new();
        for p in profiles {
            if DISABLED_PREFIXES.contains(&p.prefix) {
                continue;
            }
            map.insert(p.prefix, p);
        }

        // Nano 7G device overrides (from C# `ProfileSystem.cs`): the device
        // stores cover art as small RGB565 frames under otherwise-global prefixes.
        let mut alternates: HashMap<i32, Vec<Profile>> = HashMap::new();
        for (prefix, w, h) in [(1013, 50, 50), (1015, 58, 58), (1016, 57, 57)] {
            alternates.insert(prefix, vec![nano_alternate(prefix, w, h)]);
        }
        Ok(Self {
            profiles: map,
            alternates,
        })
    }

    /// Load an external `profiles.json` file and merge its entries,
    /// overriding any existing profiles by matching prefix.
    ///
    /// # Errors
    /// Returns `DecodeError::Profile` if the file cannot be read or parsed.
    /// NOTE: currently no production caller exercises this — it is a documented
    /// library feature (docs/adr/0003-profile-resolution-and-discovery.md); kept for
    /// library users, not dead code.
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

    /// Resolve the profile for `prefix`, preferring a Nano 7G alternate whose
    /// frame size matches `data_len` within the 256-byte trailing-padding
    /// tolerance (C# `ProfileSystem` device-override cascade). Falls back to the
    /// main profile entry when no alternate matches.
    #[must_use]
    #[allow(clippy::cast_sign_loss)]
    pub fn resolve(&self, prefix: i32, data_len: usize) -> Option<Profile> {
        if let Some(alts) = self.alternates.get(&prefix)
            && let Some(a) = alts
                .iter()
                .find(|a| data_len.abs_diff(a.frame_byte_length as usize) <= 256)
        {
            return Some(a.clone());
        }
        self.profiles.get(&prefix).cloned()
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

/// Build a Nano 7G alternate profile: small RGB565 frame, little-endian, no rotation.
fn nano_alternate(prefix: i32, width: i32, height: i32) -> Profile {
    Profile {
        prefix,
        width,
        height,
        encoding: Encoding::Rgb565,
        frame_byte_length: width * height * 2,
        little_endian: true,
        ..Default::default()
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
    fn load_builtin_has_53_profiles() {
        let db = ProfileDb::load_builtin().unwrap();
        assert_eq!(db.len(), 53, "profile 1044 must be disabled (iOpenPod #81)");
        assert!(db.get(1044).is_none(), "profile 1044 must not be active");
    }

    #[test]
    fn get_1007_returns_correct_profile() {
        let db = ProfileDb::load_builtin().unwrap();
        let p = db.get(1007).expect("profile 1007 should exist");
        assert_eq!(p.prefix, 1007);
        assert_eq!(p.width, 480);
        assert_eq!(p.height, 864);
        assert_eq!(p.encoding, crate::profile::Encoding::Rgb565);
        assert_eq!(p.frame_byte_length, 829_440);
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
        assert_eq!(all.len(), 53);
    }

    #[test]
    fn resolve_picks_nano_alternate_within_tolerance() {
        let db = ProfileDb::load_builtin().unwrap();
        // 1013 global frame is 77440 B; a 5000 B frame must resolve to the
        // Nano 7G alternate (50×50, LE, no rotation).
        let p = db.resolve(1013, 5000).expect("5000-byte 1013 frame resolves");
        assert_eq!(p.width, 50);
        assert_eq!(p.height, 50);
        assert_eq!(p.frame_byte_length, 5000);
        assert!(p.little_endian, "Nano 7G alternates are little-endian");
        assert_eq!(p.rotation, 0);
        // Within the 256-byte trailing-padding tolerance the alternate still wins.
        let p = db.resolve(1013, 5000 + 128).expect("within tolerance");
        assert_eq!(p.frame_byte_length, 5000);
        // Other Nano 7G prefixes.
        assert_eq!(db.resolve(1015, 6728).expect("1015 alternate").frame_byte_length, 6728);
        assert_eq!(db.resolve(1016, 6498).expect("1016 alternate").frame_byte_length, 6498);
    }

    #[test]
    fn resolve_falls_back_to_global_outside_tolerance() {
        let db = ProfileDb::load_builtin().unwrap();
        // Full-size global frame (77440 B) differs from the alternate (5000 B)
        // by far more than 256 → falls back to the global profile.
        let p = db.resolve(1013, 77_440).expect("full-size 1013 frame resolves");
        assert_eq!(p.width, 220);
        assert_eq!(p.height, 176);
        assert_eq!(p.frame_byte_length, 77_440);
        assert!(!p.little_endian, "global 1013 is big-endian");
        assert_eq!(p.rotation, 90);
        // Unknown prefix still resolves to None.
        assert!(db.resolve(9999, 100).is_none());
    }
}
