//! Parse content of UEFI capsule binaries
//!
//! Specific to those used by Framework. The UEFI specification does not
//! specify the structure of a capsule's content.
//use core::prelude::rust_2021::derive;

use crate::alloc::string::ToString;
use alloc::string::String;
use core::convert::TryInto;

use crate::ccgx::binary::{CCG5_PD_LEN, CCG6_PD_LEN, CCG8_PD_LEN};
use crate::ec_binary::EC_LEN;
use crate::util;

pub fn find_retimer_version(data: &[u8]) -> Option<u16> {
    let needle = b"$_RETIMER_PARAM_";
    let found = util::find_sequence(data, needle)?;
    let offset = found + 0x8 + needle.len();
    let bytes = data.get(offset..offset + 2)?;
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
    let platform = std::str::from_utf8(data.get(platform_offset..platform_offset + 5)?)
        .map(|x| x.to_string())
        .ok()?;

    let ver_offset = found + 0x10 + needle.len() - 1;
    let version = std::str::from_utf8(data.get(ver_offset..ver_offset + 5)?)
        .map(|x| x.to_string())
        .ok()?;

    Some(BiosCapsule { platform, version })
}

pub fn find_ec_in_bios_cap(data: &[u8]) -> Option<&[u8]> {
    let needle = b"$_IFLASH_EC_IMG_";
    let found = util::find_sequence(data, needle)?;
    let ec_offset = found + 0x9 + needle.len() - 1;
    data.get(ec_offset..ec_offset + EC_LEN)
}

pub fn find_pd_in_bios_cap(data: &[u8]) -> Option<&[u8]> {
    // Just search for the first couple of bytes in PD binaries
    // TODO: There's a second one but unless the capsule is bad, we can assume
    // they're the same version
    let ccg5_needle = &[0x00, 0x20, 0x00, 0x20, 0x11, 0x00];
    let ccg6_needle = &[0x00, 0x40, 0x00, 0x20, 0x11, 0x00];
    let ccg8_needle = &[0x00, 0x80, 0x00, 0x20, 0xAD, 0x0C];
    if let Some(found_pd1) = util::find_sequence(data, ccg5_needle) {
        data.get(found_pd1..found_pd1 + CCG5_PD_LEN)
    } else if let Some(found_pd1) = util::find_sequence(data, ccg6_needle) {
        data.get(found_pd1..found_pd1 + CCG6_PD_LEN)
    } else if let Some(found_pd1) = util::find_sequence(data, ccg8_needle) {
        data.get(found_pd1..found_pd1 + CCG8_PD_LEN)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // Needle found at the end of the file, without the data after it
    fn truncated_content() {
        assert!(find_retimer_version(b"$_RETIMER_PARAM_").is_none());
        assert!(find_bios_version(b"$BVDT").is_none());
        assert!(find_ec_in_bios_cap(b"$_IFLASH_EC_IMG_").is_none());
        assert!(find_pd_in_bios_cap(&[0x00, 0x20, 0x00, 0x20, 0x11, 0x00]).is_none());
        assert!(find_pd_in_bios_cap(&[0x00, 0x40, 0x00, 0x20, 0x11, 0x00]).is_none());
        assert!(find_pd_in_bios_cap(&[0x00, 0x80, 0x00, 0x20, 0xAD, 0x0C]).is_none());
    }
}
