//! Device-specific format-ID lookup tables — maps each known iPod/iPhone
//! generation to the format IDs it generates for its thumbnail caches
//! (`PhotoDB` & `ArtworkDB`).
//!
//! Synthesised from iOpenPod, `OrgZ`, libgpod, gnupod, and the 22-repo
//! research sweep. **Read-only reference** — not used during decode.

mod tables;

/// A single format entry known to a device generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceFormatInfo {
    /// Numeric format ID (the big-endian 4-byte prefix in .ithmb files).
    pub format_id: i32,
    /// Human-readable description (dimensions and encoding).
    pub description: &'static str,
}

/// A device generation and the set of format IDs it produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceProfile {
    /// Human-readable device name (e.g. "iPod Classic 5G (Video)").
    pub name: &'static str,
    /// Slice of format entries known to this device.
    pub formats: &'static [DeviceFormatInfo],
}

// ---------------------------------------------------------------------------
// Master device list (18 profiles, some sharing format tables above)
// ---------------------------------------------------------------------------

/// All known device profiles.
pub static DEVICE_PROFILES: &[DeviceProfile] = &[
    DeviceProfile {
        name: "iPod Classic 5G (Video)",
        formats: tables::CLASSIC_5G,
    },
    DeviceProfile {
        name: "iPod Classic 5.5G (Enhanced)",
        formats: tables::CLASSIC_5_5G,
    },
    DeviceProfile {
        name: "iPod Classic 6G (Thin)",
        formats: tables::CLASSIC_6G,
    },
    DeviceProfile {
        name: "iPod Nano 1G",
        formats: tables::NANO_1G,
    },
    DeviceProfile {
        name: "iPod Nano 2G",
        formats: tables::NANO_2G,
    },
    DeviceProfile {
        name: "iPod Nano 3G",
        formats: tables::NANO_3G,
    },
    DeviceProfile {
        name: "iPod Nano 4G",
        formats: tables::NANO_4G,
    },
    DeviceProfile {
        name: "iPod Nano 5G",
        formats: tables::NANO_5G,
    },
    DeviceProfile {
        name: "iPod Nano 6G",
        formats: tables::NANO_6G,
    },
    DeviceProfile {
        name: "iPod Nano 7G",
        formats: tables::NANO_7G,
    },
    DeviceProfile {
        name: "iPod Video 5G",
        formats: tables::CLASSIC_5G,
    },
    DeviceProfile {
        name: "iPod Mini 1G/2G",
        formats: tables::MINI_1G_2G,
    },
    DeviceProfile {
        name: "iPod Photo 4G",
        formats: tables::PHOTO_4G,
    },
    DeviceProfile {
        name: "iPod Touch 1G/2G",
        formats: tables::TOUCH,
    },
    DeviceProfile {
        name: "iPod Touch 3G/4G",
        formats: tables::TOUCH,
    },
    DeviceProfile {
        name: "iPhone 1G/2G",
        formats: tables::IPHONE,
    },
    DeviceProfile {
        name: "iPhone 3G/3GS",
        formats: tables::IPHONE,
    },
    DeviceProfile {
        name: "Motorola ROKR E1",
        formats: tables::ROKR_E1,
    },
];

// ---------------------------------------------------------------------------
// Public lookup functions
// ---------------------------------------------------------------------------

/// Find a device profile by name (case-insensitive substring match).
#[must_use]
pub fn find_device(name: &str) -> Option<&'static DeviceProfile> {
    let lower = name.to_ascii_lowercase();
    DEVICE_PROFILES
        .iter()
        .find(|p| p.name.to_ascii_lowercase().contains(&lower))
}

/// Search **all** device profiles for every format entry matching `format_id`.
#[must_use]
pub fn find_formats_by_id(format_id: i32) -> Vec<&'static DeviceFormatInfo> {
    let mut results: Vec<&'static DeviceFormatInfo> = Vec::new();
    for profile in DEVICE_PROFILES {
        if let Some(info) = profile.formats.iter().find(|f| f.format_id == format_id) {
            results.push(info);
        }
    }
    results
}

/// Return a reference to the complete device-profiles table.
#[must_use]
pub fn all_device_profiles() -> &'static [DeviceProfile] {
    DEVICE_PROFILES
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn has_18_device_profiles() {
        assert_eq!(DEVICE_PROFILES.len(), 18);
    }

    #[test]
    fn find_classic_5g_by_name() {
        let device = find_device("iPod Classic 5G").expect("should find Classic 5G");
        assert_eq!(device.name, "iPod Classic 5G (Video)");
        assert_eq!(device.formats.len(), 7);
    }

    #[test]
    fn find_case_insensitive() {
        let device = find_device("IPOD CLASSIC 5G").expect("case-insensitive");
        assert_eq!(device.name, "iPod Classic 5G (Video)");
    }

    #[test]
    fn nano_7g_has_two_formats() {
        let device = find_device("iPod Nano 7G").expect("Nano 7G");
        assert_eq!(device.formats.len(), 2);
        assert!(device.formats.iter().any(|f| f.format_id == 1007));
    }

    #[test]
    fn touch_3g_reuses_touch_array() {
        let t3 = find_device("iPod Touch 3G").expect("Touch 3G/4G");
        let t1 = find_device("iPod Touch 1G").expect("Touch 1G/2G");
        assert_eq!(t3.formats.len(), 8);
        assert!(
            std::ptr::eq(t3.formats, t1.formats),
            "Touch devices must share format table"
        );
    }

    #[test]
    fn iphone_3g_reuses_iphone_array() {
        let i1 = find_device("iPhone 1G").expect("iPhone 1G/2G");
        let i3 = find_device("iPhone 3G").expect("iPhone 3G/3GS");
        assert!(
            std::ptr::eq(i1.formats, i3.formats),
            "iPhone devices must share format table"
        );
    }

    #[test]
    fn video_5g_reuses_classic_5g_array() {
        let c5 = find_device("iPod Classic 5G").expect("Classic 5G");
        let v5 = find_device("iPod Video 5G").expect("Video 5G");
        assert!(
            std::ptr::eq(c5.formats, v5.formats),
            "Video 5G must share Classic 5G table"
        );
    }

    #[test]
    fn rokr_e1_has_two_formats() {
        let device = find_device("Motorola ROKR E1").expect("ROKR E1");
        assert_eq!(device.formats.len(), 2);
        assert!(device.formats.iter().any(|f| f.format_id == 2003));
    }

    #[test]
    fn nonexistent_device_returns_none() {
        assert!(find_device("iPod Shuffle").is_none());
    }

    #[test]
    fn find_1019_across_devices() {
        let results = find_formats_by_id(1019);
        assert!(!results.is_empty());
        assert!(results.iter().all(|f| f.format_id == 1019));
    }

    #[test]
    fn find_3001_in_four_devices() {
        let results = find_formats_by_id(3001);
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn nonexistent_format_returns_empty() {
        assert!(find_formats_by_id(9999).is_empty());
    }

    #[test]
    fn all_device_profiles_is_stable() {
        assert!(std::ptr::eq(all_device_profiles(), DEVICE_PROFILES));
    }

    #[test]
    fn nano_4g_has_most_formats() {
        let device = find_device("iPod Nano 4G").expect("Nano 4G");
        assert_eq!(device.formats.len(), 12);
    }

    #[test]
    fn every_device_has_formats() {
        for p in DEVICE_PROFILES {
            assert!(!p.formats.is_empty(), "{}", p.name);
        }
    }

    #[test]
    fn every_format_has_description() {
        for p in DEVICE_PROFILES {
            for f in p.formats {
                assert!(!f.description.is_empty());
            }
        }
    }
}
