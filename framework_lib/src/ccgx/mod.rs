//! Interact with Infineon (formerly Cypress) PD controllers (their firmware binaries) in the CCGx series

use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;
use num_derive::FromPrimitive;
use std::fmt;

use crate::chromium_ec::{CrosEc, EcResult};
use crate::smbios;
use crate::util::Platform;

use self::device::{FwMode, PdController, PdPort};

pub mod binary;
pub mod device;
#[cfg(feature = "hidapi")]
pub mod hid;

const FW1_METADATA_ROW: u32 = 0x1FE;
const FW1_METADATA_ROW_CCG8: u32 = 0x3FE;
const FW2_METADATA_ROW_CCG5: u32 = 0x1FF;
const FW2_METADATA_ROW_CCG6: u32 = 0x1FD;
const FW2_METADATA_ROW_CCG8: u32 = 0x3FF;
const METADATA_OFFSET: usize = 0xC0; // TODO: Is this 0x40 on ADL?
const CCG8_METADATA_OFFSET: usize = 0x80;
const CCG3_METADATA_OFFSET: usize = 0x40;
const METADATA_MAGIC: u16 = u16::from_le_bytes([b'Y', b'C']); // CY (Cypress)
const CCG8_METADATA_MAGIC: u16 = u16::from_le_bytes([b'F', b'I']); // IF (Infineon)

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct CyAcdMetadata {
    /// Offset 00: Single Byte FW Checksum
    _fw_checksum: u8,
    /// Offset 01: FW Entry Address
    _fw_entry: u32,
    /// Offset 05: Last Flash row of Bootloader or previous firmware
    boot_last_row: u16,
    /// Offset 07: Reserved
    _reserved1: [u8; 2],
    /// Offset 09: Size of Firmware
    fw_size: u32,
    /// Offset 0D: Reserved
    _reserved2: [u8; 3],
    /// Offset 10: Creator specific field
    _active_boot_app: u8,
    /// Offset 11: Creator specific field
    _boot_app_ver_status: u8,
    /// Offset 12: Creator specific field
    _boot_app_version: u16,
    /// Offset 14: Creator specific field
    _boot_app_id: u16,
    /// Offset 16: Metadata Valid field. Valid if contains "CY"
    metadata_valid: u16,
    /// Offset 18: Creator specific field
    _fw_version: u32,
    /// Offset 1C: Boot sequence number field. Boot-loader will load the valid
    ///            FW copy that has the higher sequence number associated with it
    /// Not relevant when checking the update binary file
    _boot_seq: u32,
}

// TODO: Would be nice to check the checksums
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct CyAcd2Metadata {
    /// Offset 00: App Firmware Start
    fw_start: u32,
    /// Offset 04: App Firmware Size
    fw_size: u32,
    /// Offset 08: Boot wait time
    _boot_app_id: u16,
    /// Offset 0A: Last Flash row of Bootloader or previous firmware
    /// Is (fw_start/FLASH_ROW_SIZE) - 1
    _boot_last_row: u16,
    /// Offset 0C: Verify Start Address
    _config_fw_start: u32,
    /// Offset 10: Verify Size
    _config_fw_size: u32,
    /// Offset 14: Boot sequence number field. Boot-loader will load the valid
    /// FW copy that has the higher sequence number associated with it
    /// Not relevant when checking the update binary file
    _boot_seq: u32,
    /// Offset 18: Reserved
    _reserved_1: [u32; 15],
    /// Offset 54: Version of the metadata structure
    metadata_version: u16,
    /// Offset 56: Metadata Valid field. Valid if contains ASCII "IF"
    metadata_valid: u16,
    /// Offset 58: App Fw CRC32 checksum
    _fw_crc32: u32,
    /// Offset 5C: Reserved
    _reserved_2: [u32; 8],
    /// Offset 7C: Metadata CRC32 checksum
    _md_crc32: u32,
}

#[non_exhaustive]
#[derive(Debug, PartialEq, FromPrimitive, Clone, Copy)]
pub enum SiliconId {
    Ccg3 = 0x1D00,
    Ccg5 = 0x2100,
    Ccg6Adl = 0x3000,
    Ccg6 = 0x30A0,
    Ccg8 = 0x3580,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct BaseVersion {
    /// Major part of the version. X of X.Y.Z.BB
    pub major: u8,
    /// Minor part of the version. Y of X.Y.Z.BB
    pub minor: u8,
    /// Patch part of the version. Z of X.Y.Z.BB
    pub patch: u8,
    /// Build Number part of the version. PP of X.Y.Z.BB
    pub build_number: u16,
}
impl BaseVersion {
    pub fn to_dec_string(&self) -> String {
        format!(
            "{}.{}.{}.{:0>3}",
            self.major, self.minor, self.patch, self.build_number
        )
    }
}
impl fmt::Display for BaseVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:X}.{:X}.{:X}.{:03X}",
            self.major, self.minor, self.patch, self.build_number
        )
    }
}
impl From<&[u8]> for BaseVersion {
    fn from(data: &[u8]) -> Self {
        Self {
            build_number: u16::from_le_bytes([data[0], data[1]]),
            patch: data[2],
            major: (data[3] & 0xF0) >> 4,
            minor: data[3] & 0x0F,
        }
    }
}
impl From<u32> for BaseVersion {
    fn from(data: u32) -> Self {
        Self::from(u32::to_le_bytes(data).as_slice())
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub enum Application {
    Notebook,
    Monitor,
    AA,
    Invalid,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct AppVersion {
    pub application: Application,
    /// Major part of the version. X of X.Y.Z
    pub major: u8,
    /// Minor part of the version. Y of X.Y.Z
    pub minor: u8,
    /// Curcuit part of the version. Z of X.Y.Z
    pub circuit: u8,
}

impl fmt::Display for AppVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:X}.{:X}.{:02X}", self.major, self.minor, self.circuit)
    }
}

impl From<&[u8]> for AppVersion {
    fn from(data: &[u8]) -> Self {
        let application = match &[data[1], data[0]] {
            b"nb" => Application::Notebook,
            b"md" => Application::Monitor,
            b"aa" => Application::AA,
            _ => Application::Invalid,
        };
        Self {
            application,
            circuit: data[2],
            major: (data[3] & 0xF0) >> 4,
            minor: data[3] & 0x0F,
        }
    }
}
impl From<u32> for AppVersion {
    fn from(data: u32) -> Self {
        Self::from(u32::to_le_bytes(data).as_slice())
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct ControllerVersion {
    pub base: BaseVersion,
    pub app: AppVersion,
}

#[derive(Debug, PartialEq)]
pub struct ControllerFirmwares {
    pub active_fw: FwMode,
    pub bootloader: ControllerVersion,
    pub backup_fw: ControllerVersion,
    pub main_fw: ControllerVersion,
}

impl ControllerFirmwares {
    pub fn active_fw(&self) -> ControllerVersion {
        match self.active_fw {
            FwMode::MainFw => self.main_fw,
            FwMode::BackupFw => self.backup_fw,
            FwMode::BootLoader => self.bootloader,
        }
    }

    pub fn active_fw_ver(&self) -> String {
        let active = self.active_fw();
        // On 11th Gen we modified base version instead of app version
        // And it's formatted as decimal instead of hex
        if let Some(Platform::IntelGen11) = smbios::get_platform() {
            active.base.to_dec_string()
        } else {
            active.app.to_string()
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum PdVersions {
    RightLeft((ControllerFirmwares, ControllerFirmwares)),
    Single(ControllerFirmwares),
    Many(Vec<ControllerFirmwares>),
}

/// Same as PdVersions but only the main FW
#[derive(Debug)]
pub enum MainPdVersions {
    RightLeft((ControllerVersion, ControllerVersion)),
    Single(ControllerVersion),
    Many(Vec<ControllerVersion>),
}

pub fn get_pd_controller_versions(ec: &CrosEc) -> EcResult<PdVersions> {
    let pd01 = PdController::new(PdPort::Right01, ec.clone()).get_fw_versions();
    let pd23 = PdController::new(PdPort::Left23, ec.clone()).get_fw_versions();
    let pd_back = PdController::new(PdPort::Back, ec.clone()).get_fw_versions();

    match (pd01, pd23, pd_back) {
        (Err(_), Err(_), Ok(pd_back)) => Ok(PdVersions::Single(pd_back)),
        (Ok(pd01), Ok(pd23), Err(_)) => Ok(PdVersions::RightLeft((pd01, pd23))),
        (Ok(pd01), Ok(pd23), Ok(pd_back)) => Ok(PdVersions::Many(vec![pd01, pd23, pd_back])),
        (Err(err), _, _) => Err(err),
        (_, Err(err), _) => Err(err),
    }
}

fn parse_metadata_ccg3(buffer: &[u8]) -> Option<(u32, u32)> {
    let buffer = &buffer[CCG3_METADATA_OFFSET..];
    let metadata_len = std::mem::size_of::<CyAcdMetadata>();
    let metadata: CyAcdMetadata =
        unsafe { std::ptr::read(buffer[0..metadata_len].as_ptr() as *const _) };
    trace!("Metadata: {:X?}", metadata);
    if metadata.metadata_valid == METADATA_MAGIC {
        Some((1 + metadata.boot_last_row as u32, metadata.fw_size))
    } else {
        None
    }
}

//fn parse_metadata(buffer: &[u8; 256]) -> Option<(u32, u32)> {
fn parse_metadata_cyacd(buffer: &[u8]) -> Option<(u32, u32)> {
    let buffer = &buffer[METADATA_OFFSET..];
    let metadata_len = std::mem::size_of::<CyAcdMetadata>();
    let metadata: CyAcdMetadata =
        unsafe { std::ptr::read(buffer[0..metadata_len].as_ptr() as *const _) };
    trace!("Metadata: {:X?}", metadata);
    if metadata.metadata_valid == METADATA_MAGIC {
        Some((1 + metadata.boot_last_row as u32, metadata.fw_size))
    } else {
        None
    }
}

fn parse_metadata_cyacd2(buffer: &[u8]) -> Option<(u32, u32)> {
    let buffer = &buffer[CCG8_METADATA_OFFSET..];
    let metadata_len = std::mem::size_of::<CyAcd2Metadata>();
    let metadata: CyAcd2Metadata =
        unsafe { std::ptr::read(buffer[0..metadata_len].as_ptr() as *const _) };
    trace!("Metadata: {:X?}", metadata);
    if metadata.metadata_valid == CCG8_METADATA_MAGIC {
        if metadata.metadata_version == 1 {
            Some((metadata.fw_start, metadata.fw_size))
        } else {
            println!("Unknown CCG8 metadata version");
            None
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // Make sure deriving does what I expect, properly comparing with multiple fields
    fn derive_ord() {
        let v0_0_0 = AppVersion {
            application: Application::Notebook,
            major: 0,
            minor: 0,
            circuit: 0,
        };
        let v1_0_1 = AppVersion {
            application: Application::Notebook,
            major: 1,
            minor: 0,
            circuit: 1,
        };
        let v0_1_0 = AppVersion {
            application: Application::Notebook,
            major: 0,
            minor: 1,
            circuit: 0,
        };
        let v1_1_1 = AppVersion {
            application: Application::Notebook,
            major: 1,
            minor: 1,
            circuit: 1,
        };
        assert_eq!(v0_0_0, v0_0_0.clone());
        assert!(v0_0_0 < v1_0_1);
        assert!(v0_1_0 < v1_0_1);
        assert!(v1_0_1 < v1_1_1);
        assert!(v1_1_1 > v1_0_1);
    }
}
