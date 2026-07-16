//! Profile database loading — one-time initialization of the built-in profile DB.
//!
//! This module owns the [`OnceLock`] that initializes [`ProfileDb::load_builtin`]
//! exactly once. All decoding entry points in the pipeline use [`get_db`] to
//! access the database and [`fallback_jpeg_profile`] to construct a fallback
//! encoding profile for JPEG streams whose prefix is unknown.

use crate::profile::{Encoding, Profile};
use crate::profile_db::ProfileDb;
#[cfg(feature = "logging")]
use log::{info, warn};
use std::sync::OnceLock;

/// Global profile database, initialized on first access.
static PROFILE_DB: OnceLock<ProfileDb> = OnceLock::new();

/// Returns a reference to the global profile database, loading it on first call.
pub(crate) fn get_db() -> &'static ProfileDb {
    PROFILE_DB.get_or_init(|| {
        let db = ProfileDb::load_builtin().expect("built-in profiles.json is corrupt — this is a bug in the ithmb-core binary, please file an issue at https://github.com/B67687/Ithmb-Codec/issues");
        #[cfg(feature = "logging")]
        info!("profile DB loaded: {} profiles", db.all().len());
        db
    })
}

/// Creates a fallback profile for JPEG files that have no matching entry in
/// the profile database.
#[must_use]
pub(crate) fn fallback_jpeg_profile() -> Profile {
    let profile = Profile {
        prefix: -1, // 0xFFFF
        width: 0,
        height: 0,
        encoding: Encoding::Jpeg,
        use_mhni_dimensions: true,
        ..Default::default()
    };
    #[cfg(feature = "logging")]
    warn!("fallback encoding {:?}: primary decode failed", profile.encoding);
    profile
}
