//! Profile database loading — one-time initialization of the built-in profile DB.
//!
//! This module owns the [`OnceLock`] that initializes [`ProfileDb::load_builtin`]
//! exactly once. All decoding entry points in the pipeline use [`get_db`] to
//! access the database and [`fallback_jpeg_profile`] to construct a fallback
//! encoding profile for JPEG streams whose prefix is unknown.

use crate::profile::{Encoding, Profile};
use crate::profile_db::ProfileDb;
use std::sync::OnceLock;

/// Global profile database, initialized on first access.
static PROFILE_DB: OnceLock<ProfileDb> = OnceLock::new();

/// Returns a reference to the global profile database, loading it on first call.
pub(crate) fn get_db() -> &'static ProfileDb {
    PROFILE_DB.get_or_init(|| ProfileDb::load_builtin().expect("built-in profile DB is valid"))
}

/// Creates a fallback profile for JPEG files that have no matching entry in
/// the profile database.
#[must_use]
pub(crate) fn fallback_jpeg_profile() -> Profile {
    Profile {
        prefix: -1, // 0xFFFF
        width: 0,
        height: 0,
        encoding: Encoding::Jpeg,
        use_mhni_dimensions: true,
        ..Default::default()
    }
}
