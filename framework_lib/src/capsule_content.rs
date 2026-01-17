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

pub fn find_pd_in_bios_cap(data: &[u8]) -> Option<&[u8]> {
    find_all_pds_in_bios_cap(data).into_iter().next()
}

/// PD binary signatures and their corresponding lengths
const CCG5_NEEDLE: &[u8] = &[0x00, 0x20, 0x00, 0x20, 0x11, 0x00];
const CCG6_NEEDLE: &[u8] = &[0x00, 0x40, 0x00, 0x20, 0x11, 0x00];
const CCG8_NEEDLE: &[u8] = &[0x00, 0x80, 0x00, 0x20, 0xAD, 0x0C];

/// Find all PD firmware binaries embedded in a BIOS capsule
pub fn find_all_pds_in_bios_cap(data: &[u8]) -> Vec<&[u8]> {
    let mut results = Vec::new();

    // Search for CCG5 PDs
    for offset in util::find_all_sequences(data, CCG5_NEEDLE) {
        if offset + CCG5_PD_LEN <= data.len() {
            results.push(&data[offset..offset + CCG5_PD_LEN]);
        }
    }

    // Search for CCG6 PDs
    for offset in util::find_all_sequences(data, CCG6_NEEDLE) {
        if offset + CCG6_PD_LEN <= data.len() {
            results.push(&data[offset..offset + CCG6_PD_LEN]);
        }
    }

    // Search for CCG8 PDs
    for offset in util::find_all_sequences(data, CCG8_NEEDLE) {
        if offset + CCG8_PD_LEN <= data.len() {
            results.push(&data[offset..offset + CCG8_PD_LEN]);
        }
    }

    results
}
