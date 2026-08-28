//! PhotoDB/ArtworkDB binary chunk types: magic constants, endian-aware byte-slice
//! read helpers, and all chunk header structs.
//!
//! Ported from `IthmbCodec.PhotoDb.Types` (C#).
//!
//! Split into sub-modules:
//! * `magic` - known chunk magic constants
//! * `endian` - endian-aware read helpers
//! * `headers_basic` - MHFD, MHSD, MHL, MHII header structs
//! * `headers_album` - MHBA, MHIA, MHIF, MHOD header structs
//! * `headers_thumb` - MHNI, MhodString header structs

#![allow(missing_docs)]

pub mod endian;
pub mod headers_album;
pub mod headers_basic;
pub mod headers_thumb;
pub mod magic;

pub use endian::*;
pub use headers_album::*;
pub use headers_basic::*;
pub use headers_thumb::*;
pub use magic::*;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_known_magic() {
        assert!(is_known_magic(MHFD));
        assert!(is_known_magic(MHSD));
        assert!(is_known_magic(MHL));
        assert!(is_known_magic(MHII));
        assert!(is_known_magic(MHNI));
        assert!(is_known_magic(MHBA));
        assert!(is_known_magic(MHIA));
        assert!(is_known_magic(MHIF));
        assert!(is_known_magic(MHOD));
        assert!(!is_known_magic(0xDEAD_BEEF));
        assert!(!is_known_magic(0));
    }

    #[test]
    fn test_read_u32_le() {
        let buf = [0x6d, 0x68, 0x66, 0x64, 0x00, 0x01, 0x02, 0x03];
        assert_eq!(read_u32_le(&buf, 0), 0x6466_686d);
        assert_eq!(read_u32(&buf, 0, true), 0x6466_686d);
    }

    #[test]
    fn test_read_u32_be() {
        let buf = [0x4d, 0x48, 0x46, 0x44, 0x00, 0x01, 0x02, 0x03];
        assert_eq!(read_u32_be(&buf, 0), 0x4d48_4644);
        assert_eq!(read_u32(&buf, 0, false), 0x4d48_4644);
    }

    #[test]
    fn test_read_i32() {
        let buf = [0xff, 0xff, 0xff, 0xff];
        assert_eq!(read_i32(&buf, 0, true), -1);
    }

    #[test]
    fn test_read_u16_le() {
        let buf = [0x34, 0x12];
        assert_eq!(read_u16(&buf, 0, true), 0x1234);
    }

    #[test]
    fn test_read_u16_be() {
        let buf = [0x12, 0x34];
        assert_eq!(read_u16(&buf, 0, false), 0x1234);
    }

    #[test]
    fn test_read_u32_convenience_equivalence() {
        let buf = [0x6d, 0x68, 0x66, 0x64];
        assert_eq!(read_u32_le(&buf, 0), read_u32(&buf, 0, true));
        assert_eq!(read_u32_be(&buf, 0), read_u32(&buf, 0, false));
    }

    #[test]
    fn test_mhfd_parse_le() {
        let data: &[u8] = b"mhfd\x0c\x00\x00\x00\x02\x00\x00\x00extra";
        let mut offset = 0;
        let hdr = MhfdHeader::parse(data, &mut offset, true).unwrap();
        assert_eq!(hdr.magic, MHFD);
        assert_eq!(hdr.header_size, 12);
        assert_eq!(hdr.entry_count, 2);
        assert_eq!(offset, 12);
    }

    #[test]
    fn test_mhfd_parse_be() {
        let data: &[u8] = &[0x64, 0x66, 0x68, 0x6d, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x02];
        let mut offset = 0;
        let hdr = MhfdHeader::parse(data, &mut offset, false).unwrap();
        assert_eq!(hdr.magic, MHFD);
        assert_eq!(hdr.header_size, 12);
        assert_eq!(hdr.entry_count, 2);
        assert_eq!(offset, 12);
    }

    #[test]
    fn test_mhsd_parse_le() {
        let data: &[u8] = b"mhsd\x10\x00\x00\x00\x01\x00\x04\x00\x03\x00\x00\x00";
        let mut offset = 0;
        let hdr = MhsdHeader::parse(data, &mut offset, true).unwrap();
        assert_eq!(hdr.magic, MHSD);
        assert_eq!(hdr.header_size, 16);
        assert_eq!(hdr.index, 1);
        assert_eq!(hdr.record_type, 4);
        assert_eq!(hdr.entry_count, 3);
        assert_eq!(offset, 16);
    }

    #[test]
    fn test_mhl_parse_le() {
        let data: &[u8] = b"mhli\x0c\x00\x00\x00\x03\x00\x00\x00";
        let mut offset = 0;
        let hdr = MhlHeader::parse(data, &mut offset, true).unwrap();
        assert_eq!(hdr.magic, MHL);
        assert_eq!(hdr.count, 3);
    }

    #[test]
    fn test_mhii_parse_le() {
        let data: &[u8] = b"mhii\x0c\x00\x00\x00\x2a\x00\x00\x00";
        let mut offset = 0;
        let hdr = MhiiHeader::parse(data, &mut offset, true).unwrap();
        assert_eq!(hdr.magic, MHII);
        assert_eq!(hdr.photo_id, 42);
    }

    #[test]
    fn test_mh_common_parse_le() {
        let ba: &[u8] = b"mhba\x0c\x00\x00\x00\x05\x00\x00\x00";
        let mut offset = 0;
        let ba_hdr = MhbaHeader::parse(ba, &mut offset, true).unwrap();
        assert_eq!(ba_hdr.magic, MHBA);
        assert_eq!(ba_hdr.album_id, 5);

        let ia: &[u8] = b"mhia\x0c\x00\x00\x00\x07\x00\x00\x00";
        let mut offset = 0;
        let ia_hdr = MhiaHeader::parse(ia, &mut offset, true).unwrap();
        assert_eq!(ia_hdr.magic, MHIA);
        assert_eq!(ia_hdr.artwork_id, 7);

        let mhif: &[u8] = b"mhif\x0c\x00\x00\x00\x01\x00\x00\x00";
        let mut offset = 0;
        let mhif_hdr = MhifHeader::parse(mhif, &mut offset, true).unwrap();
        assert_eq!(mhif_hdr.magic, MHIF);
        assert_eq!(mhif_hdr.info_type, 1);
    }

    #[test]
    fn test_mhod_parse_le() {
        let data: &[u8] = &[0x01, 0x00, 0x0a, 0x00, 0xff, 0xff];
        let mut offset = 0;
        let hdr = MhodHeader::parse(data, &mut offset, true).unwrap();
        assert_eq!(hdr.tag, 1);
        assert_eq!(hdr.size, 10);
        assert_eq!(offset, 4);
    }

    #[test]
    fn test_mhni_parse_inline_le() {
        let mut data = vec![0u8; 5000];
        data[0..4].copy_from_slice(b"mhni");
        data[4..8].copy_from_slice(&[36, 0, 0, 0]);
        data[16..20].copy_from_slice(&[0xfb, 0x03, 0, 0]);
        data[20..24].copy_from_slice(&[0x80, 0, 0, 0]);
        data[24..28].copy_from_slice(&[0x00, 0x10, 0, 0]);
        data[32..34].copy_from_slice(&[0xe0, 0x01]);
        data[34..36].copy_from_slice(&[0xd0, 0x02]);

        let mut offset = 0;
        let hdr = MhniHeader::parse(&data, &mut offset, true).unwrap();
        assert_eq!(hdr.magic, MHNI);
        assert_eq!(hdr.header_size, 36);
        assert_eq!(hdr.format_id, 1019);
        assert_eq!(hdr.ithmb_offset, 128);
        assert_eq!(hdr.image_size, 4096);
        assert_eq!(hdr.width, 720);
        assert_eq!(hdr.height, 480);
        assert_eq!(hdr.h_padding, 0);
        assert_eq!(hdr.v_padding, 0);
        assert_eq!(offset, 36);
    }

    #[test]
    fn test_mhni_parse_external_le() {
        let mut data = vec![0u8; 76];
        data[0..4].copy_from_slice(b"mhni");
        data[4..8].copy_from_slice(&[76, 0, 0, 0]);
        data[16..20].copy_from_slice(&[0xfb, 0x03, 0, 0]);
        data[20..24].copy_from_slice(&[0xe0, 0x01, 0x2c, 0x01]);

        let mut offset = 0;
        let hdr = MhniHeader::parse(&data, &mut offset, true).unwrap();
        assert_eq!(hdr.magic, MHNI);
        assert_eq!(hdr.header_size, 76);
        assert_eq!(hdr.format_id, 1019);
        assert_eq!(hdr.ithmb_offset, -1);
        assert_eq!(hdr.image_size, 0);
        assert_eq!(hdr.width, 0x01e0);
        assert_eq!(hdr.height, 0x012c);
    }

    #[test]
    fn test_mhfd_buffer_too_short() {
        let data: &[u8] = b"mhfd\x0c\x00";
        let mut offset = 0;
        let result = MhfdHeader::parse(data, &mut offset, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_mhod_string() {
        let data: &[u8] = b"Hello\x00world";
        let mut offset = 0;
        let s = MhodString::parse(data, &mut offset, 6).unwrap();
        assert_eq!(s.raw, b"Hello\x00");
        assert_eq!(offset, 6);
    }

    #[test]
    fn test_parse_magic_equivalence() {
        let buf = [0x6d, 0x68, 0x66, 0x64];
        let le = read_u32(&buf, 0, true);
        let be = read_u32(&buf, 0, false);
        assert_eq!(le, MHFD);
        assert_ne!(le, be);
        assert_ne!(be, MHFD);
    }
}
