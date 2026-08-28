//! Static device format tables — shared arrays for identical-format devices.

use super::DeviceFormatInfo;

// ---------------------------------------------------------------------------
// Shared format tables — identical-format devices share a single array
// ---------------------------------------------------------------------------

macro_rules! formats {
    ($($id:expr => $desc:expr),+ $(,)?) => {
        &[$(DeviceFormatInfo { format_id: $id, description: $desc }),+]
    };
}

pub(super) static CLASSIC_5G: &[DeviceFormatInfo] = formats![
    1019 => "720×480 YUV422 interlaced full-screen",
    1024 => "320×240 RGB565 photo",
    1027 => "100×100 RGB565 cover art",
    1028 => "100×100 RGB565",
    1029 => "200×200 RGB565",
    1031 => "42×42 RGB565",
    1032 => "42×37 RGB565",
];

pub(super) static CLASSIC_5_5G: &[DeviceFormatInfo] = formats![
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

pub(super) static CLASSIC_6G: &[DeviceFormatInfo] = formats![
    1024 => "320×240 RGB565 photo",
    1055 => "128×128 RGB565 cover art",
    1060 => "320×320 RGB565 cover art large",
    1061 => "56×56 RGB565 cover art small",
    1066 => "64×64 RGB565 photo",
    1067 => "720×480 YCbCr420 padded full-screen",
    1068 => "128×128 RGB565 cover art",
];

pub(super) static NANO_1G: &[DeviceFormatInfo] = formats![
    1024 => "320×240 RGB565",
    1027 => "100×100 RGB565",
];

pub(super) static NANO_2G: &[DeviceFormatInfo] = formats![
    1019 => "720×480 YUV422",
    1027 => "100×100",
    1028 => "100×100",
    1029 => "200×200",
    1032 => "42×37",
];

pub(super) static NANO_3G: &[DeviceFormatInfo] = formats![
    1066 => "64×64 RGB565",
    1067 => "720×480 YCbCr420 padded",
    1068 => "128×128",
    1071 => "240×240",
    1073 => "240×240",
    1074 => "50×50",
];

pub(super) static NANO_4G: &[DeviceFormatInfo] = formats![
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

pub(super) static NANO_5G: &[DeviceFormatInfo] = formats![
    1087 => "384×384 RGB565",
    1092 => "80×80",
    1093 => "512×512",
];

pub(super) static NANO_6G: &[DeviceFormatInfo] = formats![
    1084 => "240×240",
    1092 => "80×80",
    1093 => "512×512",
];

pub(super) static NANO_7G: &[DeviceFormatInfo] = formats![
    1007 => "480×864 RGB565 full-res",
    1010 => "240×240 RGB565 cover art",
];

// iPod Video 5G shares the same table as Classic 5G
pub(super) static MINI_1G_2G: &[DeviceFormatInfo] = formats![
    1024 => "320×240 RGB565",
    1027 => "100×100",
];

pub(super) static PHOTO_4G: &[DeviceFormatInfo] = formats![
    1013 => "220×176 RGB565 big-endian",
    1015 => "130×88 RGB565",
    1016 => "140×140",
    1019 => "720×480 YUV422",
];

// iPod Touch 1G/2G and Touch 3G/4G share the same table
pub(super) static TOUCH: &[DeviceFormatInfo] = formats![
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
pub(super) static IPHONE: &[DeviceFormatInfo] = formats![
    3001 => "256×256 RGB555",
    3002 => "128×128 RGB555",
    3003 => "64×64 RGB555",
    3004 => "56×55 RGB555",
    3005 => "320×320 RGB555",
    3008 => "640×480 RGB555",
    3009 => "160×120 RGB555",
    3011 => "80×79 RGB555",
];

pub(super) static ROKR_E1: &[DeviceFormatInfo] = formats![
    2002 => "50×50 RGB565 big-endian",
    2003 => "150×150 RGB565 big-endian",
];
