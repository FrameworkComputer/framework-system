/**
 * PD Flash
 *
 * For TGL devices the microprocessor is Infineon's CCG5
 * For ADL devices the microprocessor is Infineon's CCG6
 *
 * We build the flash binary and then embed it into the beginning of the BIOS flash.
 * Currently the flash binary is 64K but we reserved 256K.
 *
 * Row is 128 (0x80) bytes wide on CCG6 (ADL/RPL). On CCG5 (TGL) it's 0x100
 * Flash is 65536 (0x10000) bytes in size.
 * Flash has 512 (0x200) rows.
 *
 * | Row Start | Row End | Size (Rows) | Name         |
 * | 0x1FD     | 0x1FE   | 0x1         | FW1 Metadata at 0xC0 (192) inside this row |
 * | 0x1FE     | 0x1FF   | 0x1         | FW2 Metadata at 0xC0 (192) inside this row |
 *
 * FW Metadata layout (at the end of the flash 0x1FD and 0x1FE)
 *
 * | Offset | Size |              |                                                 |
 * |--------|------|--------------|-------------------------------------------------|
 * | 0x00   | 0x18 | Total Size   |                                                 |
 * | 0x00   | 0x05 | Unknown      |                                                 |
 * | 0x05   | 0x04 | Last BL Row  | LE u32 to indicate the last row of the bootloader, FW begins afterwards |
 * | 0x09   | 0x04 | FW Num Rows  | LE u32 Size of the firmware in rows             |
 * | 0x0D   | 0x08 | Unknown      |                                                 |
 * | 0x16   | 0x01 | Magic Byte 0 | Must be 0x59                                    |
 * | 0x17   | 0x01 | Magic Byte 1 | Must be 0x43                                    |
 *
 * FW Layout (not at the same location as the metadata! But metadata points there)
 *
 * | Offset | Size |              |                                                 |
 * |--------|------|--------------|-------------------------------------------------|
 * | 0x00   | 0x18 | Total Size   |                                                 |
 * | 0xE4   | 0x05 | Unknown      |                                                 |
 * | 0xE8   | 0x01 | Patch version| X.Y.ZZ ZZ part of the version                   |
 * | 0xE9   | 0x01 | Version      | X.Y.ZZ X and Y part of the version (4 bits each)|
 */
#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

use crate::ccgx::{AppVersion, BaseVersion};

const FW1_METADATA_ROW: u32 = 0x1FE;
const FW2_METADATA_ROW_CCG5: u32 = 0x1FF;
const FW2_METADATA_ROW_CCG6: u32 = 0x1FD;
const LAST_BOOTLOADER_ROW: usize = 0x05;
const FW_SIZE_OFFSET: usize = 0x09;
const METADATA_OFFSET: usize = 0xC0;
const METADATA_MAGIC_OFFSET: usize = 0x16;
const METADATA_MAGIC_1: u8 = 0x59;
const METADATA_MAGIC_2: u8 = 0x43;
const SILICON_ID_OFFSET: usize = 0xE8;
const SILICON_FAMILY_BYTE: usize = 0x02;

/// Base Version, 4 bytes long
const BASE_VERSION_OFFSET: usize = 0xE0;
/// App Version, 4 bytes long
const APP_VERSION_OFFSET: usize = 0xE4;

/// Information about all the firmware in a PD binary file
///
/// Each file has two firmwares.
/// TODO: Find out what the difference is, since they're different in size.
#[derive(Debug, PartialEq)]
pub struct PdFirmwareFile {
    pub first: PdFirmware,
    pub second: PdFirmware,
}

#[derive(Debug)]
pub enum CcgX {
    Ccg5,
    Ccg6,
}

/// Information about a single PD firmware
#[derive(Debug, PartialEq)]
pub struct PdFirmware {
    /// TODO: Find out what this is
    pub silicon_id: u16,
    pub base_version: BaseVersion,
    pub app_version: AppVersion,
    /// At which row in the file this firmware is
    pub start_row: u32,
    /// How many bytes the firmware is in size
    pub size: u32,
    /// How many bytes are in a row
    pub row_size: u32,
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
    flash_row_size: u32,
    metadata_offset: u32,
) -> Option<(u32, u32)> {
    let buffer = read_256_bytes(file_buffer, metadata_offset, flash_row_size)?;
    let metadata = &buffer[METADATA_OFFSET..];

    if (metadata[METADATA_MAGIC_OFFSET] == METADATA_MAGIC_1)
        && (metadata[METADATA_MAGIC_OFFSET + 1] == METADATA_MAGIC_2)
    {
        let fw_row_start = (metadata[LAST_BOOTLOADER_ROW] as u32)
            + ((metadata[LAST_BOOTLOADER_ROW + 1] as u32) << 8)
            + 1;
        let fw_size = (metadata[FW_SIZE_OFFSET] as u32)
            + ((metadata[FW_SIZE_OFFSET + 1] as u32) << 8)
            + ((metadata[FW_SIZE_OFFSET + 2] as u32) << 16)
            + ((metadata[FW_SIZE_OFFSET + 3] as u32) << 24);
        Some((fw_row_start, fw_size))
    } else {
        // println!("Metadata is invalid");
        None
    }
}

/// Read 256 bytes starting from a particular row
fn read_256_bytes(file_buffer: &[u8], row_no: u32, flash_row_size: u32) -> Option<Vec<u8>> {
    let file_read_pointer = (row_no * flash_row_size) as usize;
    if file_read_pointer + 256 > file_buffer.len() {
        // Overrunning the end of the file, this can happen if we read a
        // CCG6 binary with CCG5 parameters, because the CCG5 flash_row_size
        // is bigger.
        return None;
    }
    Some(file_buffer[file_read_pointer..file_read_pointer + 256].to_vec())
}

/// Read version information about FW based on a particular metadata offset
///
/// There can be multiple metadata and FW regions in the image,
/// so it's required to specify which metadata region to read from.
fn read_version(
    file_buffer: &[u8],
    flash_row_size: u32,
    metadata_offset: u32,
) -> Option<PdFirmware> {
    let (fw_row_start, fw_size) = read_metadata(file_buffer, flash_row_size, metadata_offset)?;
    let data = read_256_bytes(file_buffer, fw_row_start, flash_row_size)?;
    let base_version = BaseVersion::from(&data[BASE_VERSION_OFFSET..]);
    let app_version = AppVersion::from(&data[APP_VERSION_OFFSET..]);
    let silicon_id = &data[SILICON_ID_OFFSET..];

    let fw_silicon_id = (silicon_id[SILICON_FAMILY_BYTE] as u16)
        + ((silicon_id[SILICON_FAMILY_BYTE + 1] as u16) << 8);

    Some(PdFirmware {
        silicon_id: fw_silicon_id,
        base_version,
        app_version,
        start_row: fw_row_start,
        size: fw_size,
        row_size: flash_row_size,
    })
}

/// Parse all PD information, given a binary file (buffer)
pub fn read_versions(file_buffer: &[u8], ccgx: CcgX) -> Option<PdFirmwareFile> {
    let (flash_row_size, fw2_metadata_row) = match ccgx {
        CcgX::Ccg5 => (0x100, FW2_METADATA_ROW_CCG5),
        CcgX::Ccg6 => (0x80, FW2_METADATA_ROW_CCG6),
    };
    let first = read_version(file_buffer, flash_row_size, FW1_METADATA_ROW)?;
    let second = read_version(file_buffer, flash_row_size, fw2_metadata_row)?;

    Some(PdFirmwareFile { first, second })
}

/// Pretty print information about PD firmware
pub fn print_fw(fw: &PdFirmware) {
    let silicon_ver = format!("{:#06x}", fw.silicon_id);
    println!("  Silicon ID: {:>20}", silicon_ver);
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
    fn can_parse_ccg5_binary() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/tgl-pd-3.8.0.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let ccg5_ver = read_versions(&data, CcgX::Ccg5);
        let ccg6_ver = read_versions(&data, CcgX::Ccg6);
        assert!(ccg5_ver.is_some());
        assert!(ccg6_ver.is_none());

        assert_eq!(
            ccg5_ver,
            Some({
                PdFirmwareFile {
                    first: PdFirmware {
                        silicon_id: 0x2100,
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
                    second: PdFirmware {
                        silicon_id: 0x2100,
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
        let ccg5_ver = read_versions(&data, CcgX::Ccg5);
        let ccg6_ver = read_versions(&data, CcgX::Ccg6);
        assert!(ccg5_ver.is_none());
        assert!(ccg6_ver.is_some());

        assert_eq!(
            ccg6_ver,
            Some({
                PdFirmwareFile {
                    first: PdFirmware {
                        silicon_id: 0x3000,
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
                    second: PdFirmware {
                        silicon_id: 0x3000,
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
}
