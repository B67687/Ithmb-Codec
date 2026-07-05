//! Device-specific format-ID lookup tables — maps each known iPod/iPhone
//! generation to the format IDs it generates for its thumbnail caches
//! (`PhotoDB` & `ArtworkDB`).
//!
//! Synthesised from iOpenPod, `OrgZ`, libgpod, gnupod, and the 22-repo
//! research sweep. **Read-only reference** — not used during decode.

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
// Shared format tables — identical-format devices share a single array
// ---------------------------------------------------------------------------

macro_rules! formats {
    ($($id:expr => $desc:expr),+ $(,)?) => {
        &[$(DeviceFormatInfo { format_id: $id, description: $desc }),+]
    };
}

static CLASSIC_5G: &[DeviceFormatInfo] = formats![
    1019 => "720×480 YUV422 interlaced full-screen",
    1024 => "320×240 RGB565 photo",
    1027 => "100×100 RGB565 cover art",
    1028 => "100×100 RGB565",
    1029 => "200×200 RGB565",
    1031 => "42×42 RGB565",
    1032 => "42×37 RGB565",
];

static CLASSIC_5_5G: &[DeviceFormatInfo] = formats![
    1019 => "720×480 YUV422 interlaced full-screen",
    1024 => "320×240 RGB565 photo",
    1027 => "100×100 RGB565 cover art",
    1028 => "100×100 RGB565",
    1029 => "200×200 RGB565",
    1031 => "42×42 RGB565",
    1032 => "42×37 RGB565",
    1055 => "128×128 RGB565 cover art medium",
    1056 => "80×80 RGB565",
];

static CLASSIC_6G: &[DeviceFormatInfo] = formats![
    1024 => "320×240 RGB565 photo",
    1055 => "128×128 RGB565 cover art",
    1060 => "320×320 RGB565 cover art large",
    1061 => "56×56 RGB565 cover art small",
    1066 => "64×64 RGB565 photo",
    1067 => "720×480 YCbCr420 padded full-screen",
    1068 => "128×128 RGB565 cover art",
];

static NANO_1G: &[DeviceFormatInfo] = formats![
    1024 => "320×240 RGB565",
    1027 => "100×100 RGB565",
];

static NANO_2G: &[DeviceFormatInfo] = formats![
    1019 => "720×480 YUV422",
    1027 => "100×100",
    1028 => "100×100",
    1029 => "200×200",
    1032 => "42×37",
];

static NANO_3G: &[DeviceFormatInfo] = formats![
    1066 => "64×64 RGB565",
    1067 => "720×480 YCbCr420 padded",
    1068 => "128×128",
    1071 => "240×240",
    1073 => "240×240",
    1074 => "50×50",
];

static NANO_4G: &[DeviceFormatInfo] = formats![
    1071 => "240×240",
    1073 => "240×240",
    1074 => "50×50",
    1078 => "80×80",
    1079 => "80×80",
    1083 => "240×320",
    1084 => "240×240",
    1085 => "88×88",
    1087 => "384×384",
    1089 => "58×58",
    1092 => "80×80",
    1093 => "512×512",
];

static NANO_5G: &[DeviceFormatInfo] = formats![
    1087 => "384×384 RGB565",
    1092 => "80×80",
    1093 => "512×512",
];

static NANO_6G: &[DeviceFormatInfo] = formats![
    1084 => "240×240",
    1092 => "80×80",
    1093 => "512×512",
];

static NANO_7G: &[DeviceFormatInfo] = formats![
    1007 => "480×864 RGB565 full-res",
    1010 => "240×240 RGB565 cover art",
];

// iPod Video 5G shares the same table as Classic 5G
static MINI_1G_2G: &[DeviceFormatInfo] = formats![
    1024 => "320×240 RGB565",
    1027 => "100×100",
];

static PHOTO_4G: &[DeviceFormatInfo] = formats![
    1013 => "220×176 RGB565 big-endian",
    1015 => "130×88 RGB565",
    1016 => "140×140",
    1019 => "720×480 YUV422",
];

// iPod Touch 1G/2G and Touch 3G/4G share the same table
static TOUCH: &[DeviceFormatInfo] = formats![
    3001 => "256×256 RGB555",
    3002 => "128×128 RGB555",
    3003 => "64×64 RGB555",
    3004 => "56×55 RGB555",
    3005 => "320×320 RGB555",
    3008 => "640×480 RGB555",
    3009 => "160×120 RGB555",
    3011 => "80×79 RGB555",
];

// iPhone 1G/2G and iPhone 3G/3GS share the same table
static IPHONE: &[DeviceFormatInfo] = formats![
    3001 => "256×256 RGB555",
    3002 => "128×128 RGB555",
    3003 => "64×64 RGB555",
    3004 => "56×55 RGB555",
    3005 => "320×320 RGB555",
    3008 => "640×480 RGB555",
    3009 => "160×120 RGB555",
    3011 => "80×79 RGB555",
];

static ROKR_E1: &[DeviceFormatInfo] = formats![
    2002 => "50×50 RGB565 big-endian",
    2003 => "150×150 RGB565 big-endian",
];

// ---------------------------------------------------------------------------
// Master device list (18 profiles, some sharing format tables above)
// ---------------------------------------------------------------------------

/// All known device profiles.
pub static DEVICE_PROFILES: &[DeviceProfile] = &[
    DeviceProfile {
        name: "iPod Classic 5G (Video)",
        formats: CLASSIC_5G,
    },
    DeviceProfile {
        name: "iPod Classic 5.5G (Enhanced)",
        formats: CLASSIC_5_5G,
    },
    DeviceProfile {
        name: "iPod Classic 6G (Thin)",
        formats: CLASSIC_6G,
    },
    DeviceProfile {
        name: "iPod Nano 1G",
        formats: NANO_1G,
    },
    DeviceProfile {
        name: "iPod Nano 2G",
        formats: NANO_2G,
    },
    DeviceProfile {
        name: "iPod Nano 3G",
        formats: NANO_3G,
    },
    DeviceProfile {
        name: "iPod Nano 4G",
        formats: NANO_4G,
    },
    DeviceProfile {
        name: "iPod Nano 5G",
        formats: NANO_5G,
    },
    DeviceProfile {
        name: "iPod Nano 6G",
        formats: NANO_6G,
    },
    DeviceProfile {
        name: "iPod Nano 7G",
        formats: NANO_7G,
    },
    DeviceProfile {
        name: "iPod Video 5G",
        formats: CLASSIC_5G,
    },
    DeviceProfile {
        name: "iPod Mini 1G/2G",
        formats: MINI_1G_2G,
    },
    DeviceProfile {
        name: "iPod Photo 4G",
        formats: PHOTO_4G,
    },
    DeviceProfile {
        name: "iPod Touch 1G/2G",
        formats: TOUCH,
    },
    DeviceProfile {
        name: "iPod Touch 3G/4G",
        formats: TOUCH,
    },
    DeviceProfile {
        name: "iPhone 1G/2G",
        formats: IPHONE,
    },
    DeviceProfile {
        name: "iPhone 3G/3GS",
        formats: IPHONE,
    },
    DeviceProfile {
        name: "Motorola ROKR E1",
        formats: ROKR_E1,
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
