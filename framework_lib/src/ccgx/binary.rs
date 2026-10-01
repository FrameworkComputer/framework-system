//! Parse CCGX PD firmware binaries and extract the metadata information
//!
//! - For Framework TGL devices the microprocessor is Infineon's CCG5
//! - For Framework ADL devices the microprocessor is Infineon's CCG6
//!
//! We build the flash binary and then embed it into the beginning of the BIOS flash.
//! Currently the flash binary is 64K but we reserved 256K.
//!
//! - Row is 128 (0x80) bytes wide on CCG6 (ADL/RPL). On CCG5 (TGL) and CCG8 it's 0x100
//! - Flash is 64K (CCG6), 128K (CCG5, CCG8 CFP) or 256K (CCG8D/S) bytes in size.
//!
//! The metadata is always in the last two rows of the flash:
//!
//! | Row         | Name                                                       |
//! |-------------|------------------------------------------------------------|
//! | last row    | FW1 (Backup) Metadata in the last 0x40 (0x80 on CCG8) bytes |
//! | last row -1 | FW2 (Main) Metadata in the last 0x40 (0x80 on CCG8) bytes   |
//!
//! FW1 comes right after the bootloader, FW2 after FW1. The metadata tells
//! where exactly they are.
//!
//! FW Layout, relative to the start of the code (which is at the start of the
//! image on CCG3/5/6 and 0x500 into the image on CCG8)
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
#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

use crate::ccgx::{AppVersion, BaseVersion};
use zerocopy::byteorder::little_endian::{U16, U32};
use zerocopy::{FromBytes, KnownLayout};

use super::*;

/// Offset of the version and silicon information in the firmware image
/// This is set by the linker script
/// To find the firmware image in the binary, get the offset from the metadata.
const FW_VERSION_OFFSET: usize = 0xE0;

// There are two different sizes of rows on different CCGX chips
const SMALL_ROW: usize = 0x80;
const LARGE_ROW: usize = 0x100;

#[repr(C, packed)]
#[derive(FromBytes, KnownLayout, Debug, Copy, Clone)]
struct VersionInfo {
    base_version: U32,
    app_version: U32,
    silicon_id: U16,
    silicon_family: U16,
}

pub const CCG5_PD_LEN: usize = 0x20_000;
pub const CCG6_PD_LEN: usize = 0x20_000;
pub const CCG8_PD_LEN: usize = 0x40_000;

/// Information about all the firmware in a PD binary file
///
/// Each file has two firmwares. FW1 (backup) is a reduced firmware that only
/// makes sure the system can charge while FW2 (main) is being updated.
#[derive(Debug, PartialEq)]
pub struct PdFirmwareFile {
    /// FW1
    pub backup_fw: PdFirmware,
    /// FW2
    pub main_fw: PdFirmware,
}

/// Information about a single PD firmware
#[derive(Debug, PartialEq)]
pub struct PdFirmware {
    /// Identifies the chip, must match what the device reports
    pub silicon_id: u16,
    pub silicon_family: u16,
    pub base_version: BaseVersion,
    pub app_version: AppVersion,
    /// At which row in the file (and flash) this firmware image starts
    pub start_row: u32,
    /// How many bytes the firmware image is in size
    pub size: usize,
    /// How many bytes are in a row
    pub row_size: usize,
    /// Row in the file (and flash) that holds the metadata of this image
    pub metadata_row: u32,
}

impl PdFirmware {
    /// Number of flash rows the image occupies
    pub fn rows(&self) -> u32 {
        self.size.div_ceil(self.row_size) as u32
    }
}

/// Size of a flash row for a particular chip
pub fn flash_row_size(ccgx: SiliconFamily) -> usize {
    match ccgx {
        SiliconFamily::Ccg3 | SiliconFamily::Ccg6Adl | SiliconFamily::Ccg6 => SMALL_ROW,
        SiliconFamily::Ccg5
        | SiliconFamily::Ccg8D
        | SiliconFamily::Ccg8S
        | SiliconFamily::Ccg6Cfp
        | SiliconFamily::Ccg8Cfp => LARGE_ROW,
    }
}

// Hexdump
// 0x4359 is the metadata magic bytes (CY - Cypress)
// 0x4649 is the metadata magic bytes (IF - Infineon)
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
fn read_metadata(
    file_buffer: &[u8],
    flash_row_size: usize,
    metadata_row: u32,
    ccgx: SiliconFamily,
) -> Option<ImageLocation> {
    trace!("read_metadata @{}", metadata_row);
    let row = read_row(file_buffer, metadata_row, flash_row_size)?;
    match ccgx {
        SiliconFamily::Ccg3
        | SiliconFamily::Ccg5
        | SiliconFamily::Ccg6Adl
        | SiliconFamily::Ccg6 => parse_metadata_cyacd(row),
        SiliconFamily::Ccg8D
        | SiliconFamily::Ccg8S
        | SiliconFamily::Ccg6Cfp
        | SiliconFamily::Ccg8Cfp => parse_metadata_cyacd2(row, flash_row_size),
    }
}

/// Get a single row from the file
fn read_row(file_buffer: &[u8], row_no: u32, flash_row_size: usize) -> Option<&[u8]> {
    let start = (row_no as usize) * flash_row_size;
    let row = file_buffer.get(start..start + flash_row_size);
    if row.is_none() {
        trace!(
            "Row {} ({} bytes) is beyond the end of the file (len: {})",
            row_no,
            flash_row_size,
            file_buffer.len()
        );
    }
    row
}

/// Read version information about FW based on a particular metadata row
///
/// There can be multiple metadata and FW regions in the image,
/// so it's required to specify which metadata region to read from.
fn read_version(
    file_buffer: &[u8],
    flash_row_size: usize,
    metadata_row: u32,
    ccgx: SiliconFamily,
) -> Option<PdFirmware> {
    let location = read_metadata(file_buffer, flash_row_size, metadata_row, ccgx)?;
    trace!("Image location: {:X?}", location);

    let version_offset = (location.start_row as usize) * flash_row_size
        + location.code_offset as usize
        + FW_VERSION_OFFSET;
    let data = file_buffer.get(version_offset..)?;
    let (version_info, _) = VersionInfo::read_from_prefix(data).ok()?;

    let base_version = BaseVersion::from(version_info.base_version.get());
    let app_version = AppVersion::from(version_info.app_version.get());

    let fw_silicon_id = version_info.silicon_id.get();
    let fw_silicon_family = version_info.silicon_family.get();

    if fw_silicon_family != ccgx as u16 {
        trace!(
            "Silicon family mismatch. Expected {:#06x}, binary has {:#06x}",
            ccgx as u16,
            fw_silicon_family
        );
        return None;
    }

    Some(PdFirmware {
        silicon_id: fw_silicon_id,
        silicon_family: fw_silicon_family,
        base_version,
        app_version,
        start_row: location.start_row,
        size: location.size as usize,
        row_size: flash_row_size,
        metadata_row,
    })
}

/// Parse all PD information, given a binary file (buffer)
///
/// The binary is expected to be an image of the whole flash, so the metadata
/// is in the last two rows of the file.
pub fn read_versions(file_buffer: &[u8], ccgx: SiliconFamily) -> Option<PdFirmwareFile> {
    let flash_row_size = flash_row_size(ccgx);
    if file_buffer.len() % flash_row_size != 0 {
        trace!(
            "File size {} is not a multiple of the row size {}",
            file_buffer.len(),
            flash_row_size
        );
        return None;
    }
    let total_rows = (file_buffer.len() / flash_row_size) as u32;
    if total_rows < 2 {
        return None;
    }
    // FW1 metadata is in the last row, FW2 metadata in the one before
    let fw1_metadata_row = total_rows - 1;
    let fw2_metadata_row = total_rows - 2;

    let backup_fw = read_version(file_buffer, flash_row_size, fw1_metadata_row, ccgx)?;
    let main_fw = read_version(file_buffer, flash_row_size, fw2_metadata_row, ccgx)?;

    if backup_fw.start_row + backup_fw.rows() > main_fw.start_row
        || main_fw.start_row + main_fw.rows() > fw2_metadata_row
    {
        error!(
            "Firmware images overlap. FW1: {}+{} rows, FW2: {}+{} rows",
            backup_fw.start_row,
            backup_fw.rows(),
            main_fw.start_row,
            main_fw.rows()
        );
        return None;
    }

    Some(PdFirmwareFile { backup_fw, main_fw })
}

/// Find the silicon family whose parameters match this binary
///
/// Returns None if no or more than one family matches.
pub fn detect_family(data: &[u8]) -> Option<(SiliconFamily, PdFirmwareFile)> {
    let families = [
        SiliconFamily::Ccg3,
        SiliconFamily::Ccg5,
        SiliconFamily::Ccg6Adl,
        SiliconFamily::Ccg6,
        SiliconFamily::Ccg8D,
        SiliconFamily::Ccg8S,
        SiliconFamily::Ccg6Cfp,
        SiliconFamily::Ccg8Cfp,
    ];
    let mut found = None;
    for family in families {
        if let Some(versions) = read_versions(data, family) {
            if found.is_some() {
                error!("{:?} matched but so did an earlier family", family);
                return None;
            }
            found = Some((family, versions));
        }
    }
    found
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
    println!("  Rows:       {:>20}", fw.rows());
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
        let (family, versions) = detect_family(&data).unwrap();
        assert_eq!(family, SiliconFamily::Ccg3);

        assert_eq!(versions, {
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
                    metadata_row: 1023,
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
                    metadata_row: 1022,
                },
            }
        });
    }

    #[test]
    fn can_parse_ccg5_binary() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/tgl-pd-3.8.0.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let (family, versions) = detect_family(&data).unwrap();
        assert_eq!(family, SiliconFamily::Ccg5);

        assert_eq!(versions, {
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
                    start_row: 20,
                    size: 36352,
                    row_size: 256,
                    metadata_row: 511,
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
                    start_row: 163,
                    size: 88832,
                    row_size: 256,
                    metadata_row: 510,
                },
            }
        });
    }

    #[test]
    fn can_parse_ccg6_binary_adl() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/adl-pd-0.1.33.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let (family, versions) = detect_family(&data).unwrap();
        assert_eq!(family, SiliconFamily::Ccg6Adl);

        assert_eq!(versions, {
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
                    metadata_row: 511,
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
                    metadata_row: 510,
                },
            }
        });
    }

    #[test]
    fn can_parse_ccg6_binary_mtl() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/mtl-pd-0.0.A.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let (family, versions) = detect_family(&data).unwrap();
        assert_eq!(family, SiliconFamily::Ccg6Adl);

        assert_eq!(versions, {
            PdFirmwareFile {
                backup_fw: PdFirmware {
                    silicon_id: 0x11C0,
                    silicon_family: 0x3000,
                    base_version: BaseVersion {
                        major: 3,
                        minor: 6,
                        patch: 0,
                        build_number: 115,
                    },
                    app_version: AppVersion {
                        application: Application::Notebook,
                        major: 0,
                        minor: 0,
                        circuit: 0x0A,
                    },
                    start_row: 10,
                    size: 12288,
                    row_size: 128,
                    metadata_row: 511,
                },
                main_fw: PdFirmware {
                    silicon_id: 0x11C0,
                    silicon_family: 0x3000,
                    base_version: BaseVersion {
                        major: 3,
                        minor: 6,
                        patch: 0,
                        build_number: 115,
                    },
                    app_version: AppVersion {
                        application: Application::Notebook,
                        major: 0,
                        minor: 0,
                        circuit: 0x0A,
                    },
                    start_row: 112,
                    size: 47744,
                    row_size: 128,
                    metadata_row: 510,
                },
            }
        });
    }

    #[test]
    fn can_parse_ccg6_binary_desktop() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/dogwood-pd-0.0E.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let (family, versions) = detect_family(&data).unwrap();
        assert_eq!(family, SiliconFamily::Ccg6Adl);

        assert_eq!(versions, {
            PdFirmwareFile {
                backup_fw: PdFirmware {
                    silicon_id: 0x11C0,
                    silicon_family: 0x3000,
                    base_version: BaseVersion {
                        major: 3,
                        minor: 7,
                        patch: 0,
                        build_number: 159,
                    },
                    app_version: AppVersion {
                        application: Application::Notebook,
                        major: 0,
                        minor: 0,
                        circuit: 0x0E,
                    },
                    start_row: 10,
                    size: 9344,
                    row_size: 128,
                    metadata_row: 511,
                },
                main_fw: PdFirmware {
                    silicon_id: 0x11C0,
                    silicon_family: 0x3000,
                    base_version: BaseVersion {
                        major: 3,
                        minor: 7,
                        patch: 0,
                        build_number: 159,
                    },
                    app_version: AppVersion {
                        application: Application::Notebook,
                        major: 0,
                        minor: 0,
                        circuit: 0x0E,
                    },
                    start_row: 112,
                    size: 50816,
                    row_size: 128,
                    metadata_row: 510,
                },
            }
        });
    }

    #[test]
    fn can_parse_ccg8_binary() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/fl16-pd-0.0.03.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let (family, versions) = detect_family(&data).unwrap();
        assert_eq!(family, SiliconFamily::Ccg8D);

        assert_eq!(versions, {
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
                    start_row: 24,
                    size: 43592,
                    row_size: 0x100,
                    metadata_row: 1023,
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
                    start_row: 285,
                    size: 112816,
                    row_size: 0x100,
                    metadata_row: 1022,
                },
            }
        });
    }

    #[test]
    fn can_parse_ccg8_binary_fl16_ai300() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/fl16-ai300-pd-0.0.22.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let (family, versions) = detect_family(&data).unwrap();
        assert_eq!(family, SiliconFamily::Ccg8D);

        assert_eq!(versions, {
            PdFirmwareFile {
                backup_fw: PdFirmware {
                    silicon_id: 0x11C5,
                    silicon_family: 0x3580,
                    base_version: BaseVersion {
                        major: 3,
                        minor: 7,
                        patch: 0,
                        build_number: 407,
                    },
                    app_version: AppVersion {
                        application: Application::Notebook,
                        major: 0,
                        minor: 0,
                        circuit: 0x22,
                    },
                    start_row: 24,
                    size: 44096,
                    row_size: 0x100,
                    metadata_row: 1023,
                },
                main_fw: PdFirmware {
                    silicon_id: 0x11C5,
                    silicon_family: 0x3580,
                    base_version: BaseVersion {
                        major: 3,
                        minor: 7,
                        patch: 0,
                        build_number: 407,
                    },
                    app_version: AppVersion {
                        application: Application::Notebook,
                        major: 0,
                        minor: 0,
                        circuit: 0x22,
                    },
                    start_row: 285,
                    size: 131192,
                    row_size: 0x100,
                    metadata_row: 1022,
                },
            }
        });
    }

    #[test]
    fn can_parse_ccg8_binary_gnss() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/gn22-pd-0.0.22.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let (family, versions) = detect_family(&data).unwrap();
        assert_eq!(family, SiliconFamily::Ccg8S);

        assert_eq!(versions, {
            PdFirmwareFile {
                backup_fw: PdFirmware {
                    silicon_id: 0x11C5,
                    silicon_family: 0x3581,
                    base_version: BaseVersion {
                        major: 3,
                        minor: 7,
                        patch: 0,
                        build_number: 407,
                    },
                    app_version: AppVersion {
                        application: Application::Notebook,
                        major: 0,
                        minor: 0,
                        circuit: 0x22,
                    },
                    start_row: 24,
                    size: 42868,
                    row_size: 0x100,
                    metadata_row: 1023,
                },
                main_fw: PdFirmware {
                    silicon_id: 0x11C5,
                    silicon_family: 0x3581,
                    base_version: BaseVersion {
                        major: 3,
                        minor: 7,
                        patch: 0,
                        build_number: 407,
                    },
                    app_version: AppVersion {
                        application: Application::Notebook,
                        major: 0,
                        minor: 0,
                        circuit: 0x22,
                    },
                    start_row: 285,
                    size: 127980,
                    row_size: 0x100,
                    metadata_row: 1022,
                },
            }
        });
    }

    #[test]
    fn can_parse_ccg8_binary_cfp() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/sakura-pd-1.0.0A.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let (family, versions) = detect_family(&data).unwrap();
        assert_eq!(family, SiliconFamily::Ccg8Cfp);

        assert_eq!(versions, {
            PdFirmwareFile {
                backup_fw: PdFirmware {
                    silicon_id: 0x11CE,
                    silicon_family: 0x3E81,
                    base_version: BaseVersion {
                        major: 3,
                        minor: 8,
                        patch: 0x50,
                        build_number: 10,
                    },
                    app_version: AppVersion {
                        application: Application::Notebook,
                        major: 1,
                        minor: 0,
                        circuit: 0x0A,
                    },
                    start_row: 2,
                    size: 25400,
                    row_size: 0x100,
                    metadata_row: 511,
                },
                main_fw: PdFirmware {
                    silicon_id: 0x11CE,
                    silicon_family: 0x3E81,
                    base_version: BaseVersion {
                        major: 3,
                        minor: 8,
                        patch: 0x50,
                        build_number: 10,
                    },
                    app_version: AppVersion {
                        application: Application::Notebook,
                        major: 1,
                        minor: 0,
                        circuit: 0x0A,
                    },
                    start_row: 103,
                    size: 80372,
                    row_size: 0x100,
                    metadata_row: 510,
                },
            }
        });
    }

    #[test]
    fn can_parse_ccg6_binary_dahlia() {
        let mut pd_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pd_bin_path.push("test_bins/dahlia-0.0.0A.bin");

        let data = fs::read(pd_bin_path).unwrap();
        let (family, versions) = detect_family(&data).unwrap();
        assert_eq!(family, SiliconFamily::Ccg6Cfp);

        assert_eq!(versions, {
            PdFirmwareFile {
                backup_fw: PdFirmware {
                    silicon_id: 0x11CE,
                    silicon_family: 0x3E03,
                    base_version: BaseVersion {
                        major: 3,
                        minor: 9,
                        patch: 0,
                        build_number: 826,
                    },
                    app_version: AppVersion {
                        application: Application::Notebook,
                        major: 0,
                        minor: 0,
                        circuit: 0x0A,
                    },
                    start_row: 2,
                    size: 26112,
                    row_size: 0x100,
                    metadata_row: 511,
                },
                main_fw: PdFirmware {
                    silicon_id: 0x11CE,
                    silicon_family: 0x3E03,
                    base_version: BaseVersion {
                        major: 3,
                        minor: 9,
                        patch: 0,
                        build_number: 826,
                    },
                    app_version: AppVersion {
                        application: Application::Notebook,
                        major: 0,
                        minor: 0,
                        circuit: 0x0A,
                    },
                    start_row: 111,
                    size: 83440,
                    row_size: 0x100,
                    metadata_row: 510,
                },
            }
        });
    }
}
