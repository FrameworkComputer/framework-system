//! Parse CCGX PD firmware binaries and extract the metadata information
//!
//! - For Framework TGL devices the microprocessor is Infineon's CCG5
//! - For Framework ADL devices the microprocessor is Infineon's CCG6
//!
//! We build the flash binary and then embed it into the beginning of the BIOS flash.
//! Currently the flash binary is 64K but we reserved 256K.
//!
//! - Row is 128 (0x80) bytes wide on CCG6 (ADL/RPL). On CCG5 (TGL) it's 0x100
//! - Flash is 65536 (0x10000) bytes in size.
//! - Flash has 512 (0x200) rows.
//!
//! | Row Start | Row End | Size (Rows) | Name                                       |
//! |-----------|---------|-------------|--------------------------------------------|
//! | 0x1FD     | 0x1FE   | 0x1         | FW1 Metadata at 0xC0 (192) inside this row |
//! | 0x1FE     | 0x1FF   | 0x1         | FW2 Metadata at 0xC0 (192) inside this row |
//!
//! FW Layout (not at the same location as the metadata! But metadata points there)
//!
//! | Offset | Size |                 |                                                     |
//! |--------|------|-----------------|---------------------------------------------------- |
//! | 0xC0   | 0x20 | Customer Region | Can be customized by us                             |
//! | 0xE0   | 0x04 | Base Version    | SDK Version                                         |
//! | 0xE4   | 0x04 | App Version     | Application Version                                 |
//! | 0xE8   | 0x02 | Silicon ID      |                                                     |
//! | 0xEA   | 0x02 | Silicon Family  |                                                     |
//! | 0xEC   | 0x28 | Reserved        | Stretches into next row, so don't bother reading it |

use alloc::format;
use alloc::vec::Vec;
#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

use crate::ccgx::{AppVersion, BaseVersion};

use super::*;

/// Offset of the version and silicon information in the firmware image
/// This is set by the linker script
/// To find the firmware image in the binary, get the offset from the metadata.
const FW_VERSION_OFFSET: usize = 0xE0;

// There are two different sizes of rows on different CCGX chips
const SMALL_ROW: usize = 0x80;
const LARGE_ROW: usize = 0x100;

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct VersionInfo {
    base_version: u32,
    app_version: u32,
    silicon_id: u16,
    silicon_family: u16,
}

pub const CCG5_PD_LEN: usize = 0x20_000;
pub const CCG6_PD_LEN: usize = 0x20_000;
pub const CCG8_PD_LEN: usize = 0x40_000;

/// Information about all the firmware in a PD binary file
///
/// Each file has two firmwares.
/// TODO: Find out what the difference is, since they're different in size.
#[derive(Debug, PartialEq)]
pub struct PdFirmwareFile {
    pub backup_fw: PdFirmware,
    pub main_fw: PdFirmware,
}

/// Information about a single PD firmware
#[derive(Debug, PartialEq)]
pub struct PdFirmware {
    /// TODO: Find out what this is
    pub silicon_id: u16,
    pub silicon_family: u16,
    pub base_version: BaseVersion,
    pub app_version: AppVersion,
    /// At which row in the file this firmware is
    pub start_row: u32,
    /// How many bytes the firmware is in size
    pub size: usize,
    /// How many bytes are in a row
    pub row_size: usize,
}

// Hexdump
// 0x4359 is the metadata magic bytes
//
// FW1
// 000ff40 5d84 0040 7500 0000 8000 00be 0000 0000
// 000ff50 0000 0000 ffff 4359 0002 0000 0000 0000
// 000ff60 0000 0000 0000 0000 0000 0000 0000 0000
// FW 2
// 000ffc0 5dbf 0010 1500 0000 8000 002f 0000 0000
// 000ffd0 0001 0000 ffff 4359 0001 0000 0000 0000
// 000ffe0 0000 0000 0000 0000 0000 0000 0000 0000

/// Read metadata to find FW binary location
/// Returns row_start, fw_size
fn read_metadata(
    file_buffer: &[u8],
    flash_row_size: usize,
    metadata_offset: u32,
    ccgx: SiliconId,
) -> Option<(u32, u32)> {
    let buffer = read_256_bytes(file_buffer, metadata_offset, flash_row_size)?;
    match ccgx {
        SiliconId::Ccg3 => parse_metadata_ccg3(&buffer),
        SiliconId::Ccg5 | SiliconId::Ccg6Adl | SiliconId::Ccg6 => parse_metadata_cyacd(&buffer),
        SiliconId::Ccg8 => parse_metadata_cyacd2(&buffer)
            .map(|(fw_row_start, fw_size)| (fw_row_start / (flash_row_size as u32), fw_size)),
    }
}

/// Read 256 bytes starting from a particular row
fn read_256_bytes(file_buffer: &[u8], row_no: u32, flash_row_size: usize) -> Option<Vec<u8>> {
    let file_read_pointer = (row_no as usize) * flash_row_size;
    let file_len = file_buffer.len();
    // Try to read as much as we can
    let read_len = if file_read_pointer + LARGE_ROW <= file_len {
        LARGE_ROW
    } else if file_read_pointer + SMALL_ROW <= file_len {
        SMALL_ROW
    } else {
        // Overrunning the end of the file, this can happen if we read a
        // CCG6 binary with CCG5 parameters, because the CCG5 flash_row_size
        // is bigger.
        return None;
    };
    Some(file_buffer[file_read_pointer..file_read_pointer + read_len].to_vec())
}

/// Read version information about FW based on a particular metadata offset
///
/// There can be multiple metadata and FW regions in the image,
/// so it's required to specify which metadata region to read from.
fn read_version(
    file_buffer: &[u8],
    flash_row_size: usize,
    metadata_offset: u32,
    ccgx: SiliconId,
) -> Option<PdFirmware> {
    let (fw_row_start, fw_size) =
        read_metadata(file_buffer, flash_row_size, metadata_offset, ccgx)?;
    let data = read_256_bytes(file_buffer, fw_row_start, flash_row_size)?;
    trace!("First row of firmware: {:X?}", data);
    let data = &data[FW_VERSION_OFFSET..];

    let version_len = std::mem::size_of::<VersionInfo>();
    let version_info: VersionInfo =
        unsafe { std::ptr::read(data[0..version_len].as_ptr() as *const _) };

    let base_version = BaseVersion::from(version_info.base_version);
    let app_version = AppVersion::from(version_info.app_version);

    let fw_silicon_id = version_info.silicon_id;
    let fw_silicon_family = version_info.silicon_family;

    Some(PdFirmware {
        silicon_id: fw_silicon_id,
        silicon_family: fw_silicon_family,
        base_version,
        app_version,
        start_row: fw_row_start,
        size: fw_size as usize,
        row_size: flash_row_size,
    })
}

/// Parse all PD information, given a binary file (buffer)
pub fn read_versions(file_buffer: &[u8], ccgx: SiliconId) -> Option<PdFirmwareFile> {
    let (flash_row_size, f1_metadata_row, fw2_metadata_row) = match ccgx {
        SiliconId::Ccg3 => (SMALL_ROW, 0x03FF, 0x03FE),
        SiliconId::Ccg5 => (LARGE_ROW, FW1_METADATA_ROW, FW2_METADATA_ROW_CCG5),
        SiliconId::Ccg6Adl => (SMALL_ROW, FW1_METADATA_ROW, FW2_METADATA_ROW_CCG6),
        SiliconId::Ccg6 => (SMALL_ROW, FW1_METADATA_ROW, FW2_METADATA_ROW_CCG6),
        SiliconId::Ccg8 => (LARGE_ROW, FW1_METADATA_ROW_CCG8, FW2_METADATA_ROW_CCG8),
    };
    let backup_fw = read_version(file_buffer, flash_row_size, f1_metadata_row, ccgx)?;
    let main_fw = read_version(file_buffer, flash_row_size, fw2_metadata_row, ccgx)?;

    Some(PdFirmwareFile { backup_fw, main_fw })
}

/// Pretty print information about PD firmware
pub fn print_fw(fw: &PdFirmware) {
    let silicon_id = format!("{:#06x}", fw.silicon_id);
    let silicon_family = format!("{:#06x}", fw.silicon_family);
    println!("  Silicon ID: {:>20}", silicon_id);
    println!("  Silicon Family: {:>16}", silicon_family);
    // TODO: Why does the padding not work? I shouldn't have to manually pad it
    println!("  Version:                  {:>20}", fw.app_version);
    println!("  Base Ver:                 {:>20}", fw.base_version);
    println!("  Row size:   {:>20} B", fw.row_size);
    println!("  Start Row:  {:>20}", fw.start_row);
    println!("  Rows:       {:>20}", fw.size / fw.row_size);
    println!("  Size:       {:>20} B", fw.size);
    println!("  Size:       {:>20} KB", fw.size / 1024);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ccgx::Application;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn can_parse_ccg3_binary() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/dp-pd-3.0.17.100.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let ccg3_ver = read_versions(&data, SiliconId::Ccg3);
        let ccg5_ver = read_versions(&data, SiliconId::Ccg5);
        let ccg6_ver = read_versions(&data, SiliconId::Ccg6);
        let ccg8_ver = read_versions(&data, SiliconId::Ccg8);
        assert!(ccg3_ver.is_some());
        assert!(ccg5_ver.is_none());
        assert!(ccg6_ver.is_none());
        assert!(ccg8_ver.is_none());

        assert_eq!(
            ccg3_ver,
            Some({
                PdFirmwareFile {
                    backup_fw: PdFirmware {
                        silicon_id: 0x11AD,
                        silicon_family: 0x1D00,
                        base_version: BaseVersion {
                            major: 3,
                            minor: 0,
                            patch: 17,
                            build_number: 100,
                        },
                        app_version: AppVersion {
                            application: Application::AA,
                            major: 0,
                            minor: 0,
                            circuit: 2,
                        },
                        start_row: 48,
                        size: 58624,
                        row_size: 128,
                    },
                    main_fw: PdFirmware {
                        silicon_id: 0x11AD,
                        silicon_family: 0x1D00,
                        base_version: BaseVersion {
                            major: 3,
                            minor: 0,
                            patch: 17,
                            build_number: 100,
                        },
                        app_version: AppVersion {
                            application: Application::AA,
                            major: 0,
                            minor: 0,
                            circuit: 2,
                        },
                        start_row: 512,
                        size: 58624,
                        row_size: 128,
                    },
                }
            })
        );
    }

    #[test]
    fn can_parse_ccg5_binary() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/tgl-pd-3.8.0.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let ccg3_ver = read_versions(&data, SiliconId::Ccg3);
        let ccg5_ver = read_versions(&data, SiliconId::Ccg5);
        let ccg6_ver = read_versions(&data, SiliconId::Ccg6);
        let ccg8_ver = read_versions(&data, SiliconId::Ccg8);
        assert!(ccg3_ver.is_none());
        assert!(ccg5_ver.is_some());
        assert!(ccg6_ver.is_none());
        assert!(ccg8_ver.is_none());

        assert_eq!(
            ccg5_ver,
            Some({
                PdFirmwareFile {
                    backup_fw: PdFirmware {
                        silicon_id: 0x11B1,
                        silicon_family: 0x2100,
                        base_version: BaseVersion {
                            major: 3,
                            minor: 4,
                            patch: 0,
                            build_number: 2575,
                        },
                        app_version: AppVersion {
                            application: Application::Notebook,
                            major: 3,
                            minor: 8,
                            circuit: 0,
                        },
                        start_row: 163,
                        size: 88832,
                        row_size: 256,
                    },
                    main_fw: PdFirmware {
                        silicon_id: 0x11B1,
                        silicon_family: 0x2100,
                        base_version: BaseVersion {
                            major: 3,
                            minor: 4,
                            patch: 0,
                            build_number: 2575,
                        },
                        app_version: AppVersion {
                            application: Application::Notebook,
                            major: 3,
                            minor: 8,
                            circuit: 0,
                        },
                        start_row: 20,
                        size: 36352,
                        row_size: 256,
                    },
                }
            })
        );
    }

    #[test]
    fn can_parse_ccg6_binary() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/adl-pd-0.1.33.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let ccg3_ver = read_versions(&data, SiliconId::Ccg3);
        let ccg5_ver = read_versions(&data, SiliconId::Ccg5);
        let ccg6_ver = read_versions(&data, SiliconId::Ccg6);
        let ccg8_ver = read_versions(&data, SiliconId::Ccg8);
        assert!(ccg3_ver.is_none());
        assert!(ccg5_ver.is_none());
        assert!(ccg6_ver.is_some());
        assert!(ccg8_ver.is_none());

        assert_eq!(
            ccg6_ver,
            Some({
                PdFirmwareFile {
                    backup_fw: PdFirmware {
                        silicon_id: 0x11C0,
                        silicon_family: 0x3000,
                        base_version: BaseVersion {
                            major: 3,
                            minor: 4,
                            patch: 0,
                            build_number: 425,
                        },
                        app_version: AppVersion {
                            application: Application::Notebook,
                            major: 0,
                            minor: 1,
                            circuit: 33,
                        },
                        start_row: 22,
                        size: 12160,
                        row_size: 128,
                    },
                    main_fw: PdFirmware {
                        silicon_id: 0x11C0,
                        silicon_family: 0x3000,
                        base_version: BaseVersion {
                            major: 3,
                            minor: 4,
                            patch: 0,
                            build_number: 425,
                        },
                        app_version: AppVersion {
                            application: Application::Notebook,
                            major: 0,
                            minor: 1,
                            circuit: 33,
                        },
                        start_row: 118,
                        size: 49408,
                        row_size: 128,
                    },
                }
            })
        );
    }

    #[test]
    fn can_parse_ccg8_binary() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/fl16-pd-0.0.03.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let ccg3_ver = read_versions(&data, SiliconId::Ccg3);
        let ccg5_ver = read_versions(&data, SiliconId::Ccg5);
        let ccg6_ver = read_versions(&data, SiliconId::Ccg6);
        let ccg8_ver = read_versions(&data, SiliconId::Ccg8);
        assert!(ccg3_ver.is_none());
        assert!(ccg5_ver.is_none());
        assert!(ccg6_ver.is_none());
        assert!(ccg8_ver.is_some());

        assert_eq!(
            ccg8_ver,
            Some({
                PdFirmwareFile {
                    backup_fw: PdFirmware {
                        silicon_id: 0x11C5,
                        silicon_family: 0x3580,
                        base_version: BaseVersion {
                            major: 3,
                            minor: 6,
                            patch: 0,
                            build_number: 160,
                        },
                        app_version: AppVersion {
                            application: Application::Notebook,
                            major: 0,
                            minor: 0,
                            circuit: 3,
                        },
                        start_row: 290,
                        size: 111536,
                        row_size: 0x100,
                    },
                    main_fw: PdFirmware {
                        silicon_id: 0x11C5,
                        silicon_family: 0x3580,
                        base_version: BaseVersion {
                            major: 3,
                            minor: 6,
                            patch: 0,
                            build_number: 160,
                        },
                        app_version: AppVersion {
                            application: Application::Notebook,
                            major: 0,
                            minor: 0,
                            circuit: 3,
                        },
                        start_row: 29,
                        size: 42312,
                        row_size: 0x100,
                    },
                }
            })
        );
    }
}
