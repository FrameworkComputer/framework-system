//! Parse Chrome EC binaries and get their metadata
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;

const CROS_EC_IMAGE_DATA_COOKIE1: u32 = 0xce778899;
const CROS_EC_IMAGE_DATA_COOKIE2: u32 = 0xceaabbdd;
// Absolute offset of the version struct inside the entire EC binary
// Legacy
// const EC_VERSION_OFFSET: usize = 0x1158; // Bootloader?
const EC_RO_VER_OFFSET: usize = 0x2430;
const EC_RW_VER_OFFSET: usize = 0x402f0;
// Zephyr
const EC_RO_VER_OFFSET_ZEPHYR: usize = 0x00180;
const EC_RW_VER_OFFSET_ZEPHYR: usize = 0x40140;
pub const EC_LEN: usize = 0x8_0000;

use regex;

#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

// Defined in EC code as `struct image_data` in include/cros_version.h
#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
struct _ImageVersionData {
    cookie1: u32,
    version: [u8; 32],
    size: u32,
    rollback_version: u32,
    cookie2: u32,
}
/// Version Information about an EC FW binary
#[derive(Debug, PartialEq)]
pub struct ImageVersionData {
    /// Full version string, example: hx30_v0.0.1-7a61a89
    pub version: String,
    pub details: ImageVersionDetails,
    /// TODO: Find out exactly what this is
    pub size: u32,
    /// TODO: Find out exactly what this is
    pub rollback_version: u32,
}

#[derive(Debug, PartialEq)]
pub struct ImageVersionDetails {
    /// Just the platform/board name, example: hx30
    pub platform: String,
    /// Major part of the version. X of X.Y.Z
    pub major: u32,
    /// Minor part of the version. X of X.Y.Z
    pub minor: u32,
    /// Patch part of the version. X of X.Y.Z
    pub patch: u32,
    /// Commit hash the firmware was built from
    pub commit: String,
}

/// Print pretty information about the EC version
pub fn print_ec_version(ver: &ImageVersionData, ro: bool) {
    println!("EC");
    println!("  Type:       {:>20}", if ro { "RO" } else { "RW" });
    println!("  Version:    {:>20}", ver.version);
    println!("  RollbackVer:{:>20}", ver.rollback_version);
    println!("  Platform:   {:>20}", ver.details.platform);
    let version = format!(
        "{}.{}.{}",
        ver.details.major, ver.details.minor, ver.details.patch
    );
    println!("  Version:    {:>20}", version);
    println!("  Commit:     {:>20}", ver.details.commit);
    println!("  Size:       {:>20} B", ver.size);
    println!("  Size:       {:>20} KB", ver.size / 1024);
}

fn parse_ec_version(data: &_ImageVersionData) -> Option<ImageVersionData> {
    let version = std::str::from_utf8(&data.version)
        .ok()?
        .trim_end_matches(char::from(0));
    Some(ImageVersionData {
        version: version.to_string(),
        size: data.size,
        rollback_version: data.rollback_version,
        details: parse_ec_version_str(version)?,
    })
}

/// Parse the EC version string into its components
///
/// # Examples
///
/// ```
/// use framework_lib::ec_binary::*;
/// // Legacy EC
/// let ver = parse_ec_version_str("hx30_v0.0.1-7a61a89");
/// assert_eq!(ver, Some(ImageVersionDetails {
///     platform: "hx30".to_string(),
///     major: 0,
///     minor: 0,
///     patch: 1,
///     commit: "7a61a89".to_string(),
/// }));
///
/// // Zephyr based EC 2023
/// let ver = parse_ec_version_str("lotus_v3.2.103876-ec:a3a7cb,os:");
/// assert_eq!(ver, Some(ImageVersionDetails {
///     platform: "lotus".to_string(),
///     major: 3,
///     minor: 2,
///     patch: 103876,
///     commit: "a3a7cb".to_string(),
/// }));
///
/// // Zephyr based EC 2024
/// let ver = parse_ec_version_str("lotus-0.0.0-c6c7ac3");
/// assert_eq!(ver, Some(ImageVersionDetails {
///     platform: "lotus".to_string(),
///     major: 0,
///     minor: 0,
///     patch: 0,
///     commit: "c6c7ac3".to_string(),
/// }));
/// ```
pub fn parse_ec_version_str(version: &str) -> Option<ImageVersionDetails> {
    debug!("Trying to parse version: {:?}", version);
    let re = regex::Regex::new(r"([a-z0-9]+)(_v|-)([0-9])\.([0-9])\.([0-9]+)-(ec:)?([0-9a-f]+)")
        .unwrap();
    let caps = re.captures(version)?;
    let platform = caps.get(1)?.as_str().to_string();
    // Skipping second
    let major = caps.get(3)?.as_str().parse::<u32>().ok()?;
    let minor = caps.get(4)?.as_str().parse::<u32>().ok()?;
    let patch = caps.get(5)?.as_str().parse::<u32>().ok()?;
    // Skipping sixth
    let commit = caps.get(7)?.as_str().to_string();

    Some(ImageVersionDetails {
        platform,
        major,
        minor,
        patch,
        commit,
    })
}

/// Parse version information from EC FW image buffer
pub fn read_ec_version(data: &[u8], ro: bool) -> Option<ImageVersionData> {
    // First try to find the legacy EC version
    let offset = if ro {
        EC_RO_VER_OFFSET
    } else {
        EC_RW_VER_OFFSET
    };
    if data.len() < offset + core::mem::size_of::<_ImageVersionData>() {
        return None;
    }
    let v: _ImageVersionData = unsafe { std::ptr::read(data[offset..].as_ptr() as *const _) };
    if v.cookie1 != CROS_EC_IMAGE_DATA_COOKIE1 {
        debug!("Failed to find legacy Cookie 1. Found: {:X?}", {
            v.cookie1
        });
    } else if v.cookie2 != CROS_EC_IMAGE_DATA_COOKIE2 {
        debug!("Failed to find legacy Cookie 2. Found: {:X?}", {
            v.cookie2
        });
    } else {
        return parse_ec_version(&v);
    }

    // If not present, find Zephyr EC version
    let offset_zephyr = if ro {
        EC_RO_VER_OFFSET_ZEPHYR
    } else {
        EC_RW_VER_OFFSET_ZEPHYR
    };
    if data.len() < offset_zephyr + core::mem::size_of::<_ImageVersionData>() {
        return None;
    }
    let v: _ImageVersionData =
        unsafe { std::ptr::read(data[offset_zephyr..].as_ptr() as *const _) };
    if v.cookie1 != CROS_EC_IMAGE_DATA_COOKIE1 {
        debug!("Failed to find Zephyr Cookie 1. Found: {:X?}", {
            v.cookie1
        });
    } else if v.cookie2 != CROS_EC_IMAGE_DATA_COOKIE2 {
        debug!("Failed to find Zephyr Cookie 2. Found: {:X?}", {
            v.cookie2
        });
    } else {
        return parse_ec_version(&v);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    // TODO: Perhaps put the binary hex data here and test it all
    #[test]
    fn can_parse() {
        let ver_chars: &[u8] = b"hx30_v0.0.1-7a61a89\0\0\0\0\0\0\0\0\0\0\0\0\0";
        let data = _ImageVersionData {
            cookie1: CROS_EC_IMAGE_DATA_COOKIE1,
            version: ver_chars.try_into().unwrap(),
            size: 2868,
            rollback_version: 0,
            cookie2: CROS_EC_IMAGE_DATA_COOKIE1,
        };
        debug_assert_eq!(
            parse_ec_version(&data),
            Some(ImageVersionData {
                version: "hx30_v0.0.1-7a61a89".to_string(),
                size: 2868,
                rollback_version: 0,
                details: ImageVersionDetails {
                    platform: "hx30".to_string(),
                    major: 0,
                    minor: 0,
                    patch: 1,
                    commit: "7a61a89".to_string(),
                }
            })
        );
    }

    #[test]
    fn can_parse_adl_ec() {
        let mut ec_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        ec_bin_path.push("test_bins/adl-ec-0.0.1.bin");
        let data = fs::read(ec_bin_path).unwrap();
        let ver = read_ec_version(&data, false);
        assert_eq!(
            ver,
            Some({
                ImageVersionData {
                    version: "hx30_v0.0.1-7a61a89".to_string(),
                    details: ImageVersionDetails {
                        platform: "hx30".to_string(),
                        major: 0,
                        minor: 0,
                        patch: 1,
                        commit: "7a61a89".to_string(),
                    },
                    size: 136900,
                    rollback_version: 0,
                }
            })
        );
    }

    #[test]
    fn can_parse_amd_fl13_ec() {
        let mut ec_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        ec_bin_path.push("test_bins/amd-fl13-ec-3.05.bin");
        let data = fs::read(ec_bin_path).unwrap();
        let expected = Some({
            ImageVersionData {
                version: "azalea_v3.4.113353-ec:b4c1fb,os".to_string(),
                details: ImageVersionDetails {
                    platform: "azalea".to_string(),
                    major: 3,
                    minor: 4,
                    patch: 113353,
                    commit: "b4c1fb".to_string(),
                },
                size: 258048,
                rollback_version: 0,
            }
        });
        assert_eq!(expected, read_ec_version(&data, false));
        assert_eq!(expected, read_ec_version(&data, true));
    }

    #[test]
    fn can_parse_amd_fl16_ec() {
        let mut ec_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        ec_bin_path.push("test_bins/amd-fl16-ec-3.03.bin");
        let data = fs::read(ec_bin_path).unwrap();
        let expected = Some({
            ImageVersionData {
                version: "lotus_v3.4.113353-ec:b4c1fb,os:".to_string(),
                details: ImageVersionDetails {
                    platform: "lotus".to_string(),
                    major: 3,
                    minor: 4,
                    patch: 113353,
                    commit: "b4c1fb".to_string(),
                },
                size: 258048,
                rollback_version: 0,
            }
        });
        assert_eq!(expected, read_ec_version(&data, false));
        assert_eq!(expected, read_ec_version(&data, true));
    }

    #[test]
    // Make sure it doesn't crash when reading an invalid binary
    // Cargo.toml is significantly smaller than ec.bin
    fn fails_cargo_toml() {
        let mut ec_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        ec_bin_path.push("Cargo.toml");
        let data = fs::read(ec_bin_path).unwrap();
        assert_eq!(None, read_ec_version(&data, false));
        assert_eq!(None, read_ec_version(&data, true));
    }

    #[test]
    // Make sure it doesn't crash when reading an invalid binary
    // winux.bin is slightly larger than ec.bin
    fn fails_winux() {
        let mut ec_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        ec_bin_path.push("test_bins/winux.bin");
        let data = fs::read(ec_bin_path).unwrap();
        assert_eq!(None, read_ec_version(&data, false));
        assert_eq!(None, read_ec_version(&data, true));
    }
}
