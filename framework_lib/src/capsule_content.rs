//! Parse content of UEFI capsule binaries
//!
//! Specific to those used by Framework. The UEFI specification does not
//! specify the structure of a capsule's content.
//use core::prelude::rust_2021::derive;

use crate::alloc::string::ToString;
use alloc::string::String;
use core::convert::TryInto;

use crate::ccgx::binary::{detect_family, PD_FLASH_SIZES};
use crate::ec_binary::EC_LEN;
use crate::util;

pub fn find_retimer_version(data: &[u8]) -> Option<u16> {
    let needle = b"$_RETIMER_PARAM_";
    let found = util::find_sequence(data, needle)?;
    let offset = found + 0x8 + needle.len();
    let bytes = &data[offset..offset + 2];
    Some(u16::from_le_bytes(bytes.try_into().ok()?))
}

pub struct BiosCapsule {
    pub platform: String,
    pub version: String,
}

pub fn find_bios_version(data: &[u8]) -> Option<BiosCapsule> {
    let needle = b"$BVDT";
    let found = util::find_sequence(data, needle)?;

    // One of: GFW30, HFW3T, HFW30, IFR30, KFM30, JFP30, LFK30, IFGA3, IFGP6, LFR20, LFSP0
    let platform_offset = found + 0xA + needle.len() - 1;
    let platform = std::str::from_utf8(&data[platform_offset..platform_offset + 5])
        .map(|x| x.to_string())
        .ok()?;

    let ver_offset = found + 0x10 + needle.len() - 1;
    let version = std::str::from_utf8(&data[ver_offset..ver_offset + 5])
        .map(|x| x.to_string())
        .ok()?;

    Some(BiosCapsule { platform, version })
}

pub fn find_ec_in_bios_cap(data: &[u8]) -> Option<&[u8]> {
    let needle = b"$_IFLASH_EC_IMG_";
    let found = util::find_sequence(data, needle)?;
    let ec_offset = found + 0x9 + needle.len() - 1;
    Some(&data[ec_offset..ec_offset + EC_LEN])
}

/// Find the PD firmware image in a BIOS capsule
///
/// The capsule reserves more space for the image than the chip's flash has,
/// the rest is padding. The returned slice is exactly one flash in size, so
/// the metadata is in its last two rows.
pub fn find_pd_in_bios_cap(data: &[u8]) -> Option<&[u8]> {
    // Just search for the first couple of bytes in PD binaries
    // TODO: There's a second one but unless the capsule is bad, we can assume
    // they're the same version
    let ccg5_needle: &[u8] = &[0x00, 0x20, 0x00, 0x20, 0x11, 0x00];
    let ccg6_needle: &[u8] = &[0x00, 0x40, 0x00, 0x20, 0x11, 0x00];
    let ccg8_needle: &[u8] = &[0x00, 0x80, 0x00, 0x20, 0xAD, 0x0C];
    // A needle can match unrelated data, so make sure a valid image follows
    // and otherwise move on to the next needle
    [ccg5_needle, ccg6_needle, ccg8_needle]
        .into_iter()
        .find_map(|needle| {
            let start = util::find_sequence(data, needle)?;
            // Try the smallest size first. A bigger slice would end in padding
            // instead of the metadata rows.
            PD_FLASH_SIZES
                .into_iter()
                .filter_map(|size| data.get(start..start + size))
                .find(|image| detect_family(image).is_some())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ccgx::SiliconFamily;
    use std::fs;
    use std::path::PathBuf;

    /// Build a buffer that looks like a BIOS capsule with a PD image inside:
    /// unrelated data, the image, 0xFF padding up to the reserved size, more data
    fn embed_in_capsule(image: &[u8], reserved: usize) -> Vec<u8> {
        let prefix_len = 0x1234;
        let mut cap = vec![0xAB; prefix_len];
        cap.extend_from_slice(image);
        cap.resize(prefix_len + reserved, 0xFF);
        cap.extend_from_slice(&[0xCD; 0x800]);
        cap
    }

    fn read_test_bin(name: &str) -> Vec<u8> {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("test_bins");
        path.push(name);
        fs::read(path).unwrap()
    }

    #[test]
    fn find_ccg6_pd_in_capsule() {
        // The MTL 3.07 BIOS capsule ships this 64K image in a 128K region
        let image = read_test_bin("mtl-pd-0.0.A.bin");
        assert_eq!(image.len(), 0x10_000);
        let cap = embed_in_capsule(&image, 0x20_000);

        let found = find_pd_in_bios_cap(&cap).unwrap();
        assert_eq!(found, &image[..]);
        let (family, versions) = detect_family(found).unwrap();
        assert_eq!(family, SiliconFamily::Ccg6Adl);
        assert_eq!(versions, detect_family(&image).unwrap().1);
    }

    #[test]
    fn find_ccg8_cfp_pd_in_capsule() {
        // The PTL BIOS capsules ship this 128K image in a 256K region
        let image = read_test_bin("sakura-pd-1.0.0A.bin");
        assert_eq!(image.len(), 0x20_000);
        let cap = embed_in_capsule(&image, 0x40_000);

        let found = find_pd_in_bios_cap(&cap).unwrap();
        assert_eq!(found, &image[..]);
        let (family, versions) = detect_family(found).unwrap();
        assert_eq!(family, SiliconFamily::Ccg8Cfp);
        assert_eq!(versions, detect_family(&image).unwrap().1);
    }

    #[test]
    fn find_ccg8_pd_in_capsule() {
        // 256K image, the region is exactly as big as the flash
        let image = read_test_bin("fl16-ai300-pd-0.0.22.bin");
        assert_eq!(image.len(), 0x40_000);
        let cap = embed_in_capsule(&image, 0x40_000);

        let found = find_pd_in_bios_cap(&cap).unwrap();
        assert_eq!(found, &image[..]);
        assert_eq!(detect_family(found).unwrap().0, SiliconFamily::Ccg8D);
    }
}
