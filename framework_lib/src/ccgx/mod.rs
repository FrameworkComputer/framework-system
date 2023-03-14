//! Interact with Infineon (formerly Cypress) PD controllers (their firmware binaries) in the CCGx series

#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;
use num_derive::FromPrimitive;
use std::fmt;

use crate::chromium_ec::EcResult;

use self::device::{PdController, PdPort};

pub mod binary;
pub mod device;
#[cfg(not(feature = "uefi"))]
pub mod hid;

const FW1_METADATA_ROW: u32 = 0x1FE;
const FW2_METADATA_ROW_CCG5: u32 = 0x1FF;
const FW2_METADATA_ROW_CCG6: u32 = 0x1FD;
const METADATA_OFFSET: usize = 0xC0; // TODO: Is this 0x40 on ADL?
const METADATA_MAGIC: u16 = u16::from_le_bytes([b'Y', b'C']); // CY (Cypress)

#[repr(packed)]
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

#[non_exhaustive]
#[derive(Debug, PartialEq, FromPrimitive, Clone, Copy)]
pub enum SiliconId {
    Ccg5 = 0x2100,
    Ccg6 = 0x3000,
}

#[derive(Debug, PartialEq)]
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
impl fmt::Display for BaseVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{:X}",
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

#[derive(Debug, PartialEq)]
pub enum Application {
    Notebook,
    Monitor,
    Invalid,
}

#[derive(Debug, PartialEq)]
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
        write!(f, "{}.{}.{:X}", self.major, self.minor, self.circuit)
    }
}

impl From<&[u8]> for AppVersion {
    fn from(data: &[u8]) -> Self {
        let application = if data[0] == 0x62 && data[1] == 0x6e {
            Application::Notebook // ASCII "nb"
        } else if data[0] == 0x64 && data[1] == 0x6d {
            Application::Monitor // ASCII "md"
        } else {
            debug_assert!(
                false,
                "Couldn't parse application 0x{:X}, 0x{:X}",
                data[0], data[1]
            );
            Application::Invalid
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

#[derive(Debug, PartialEq)]
pub struct ControllerVersion {
    pub base: BaseVersion,
    pub app: AppVersion,
}

#[derive(Debug, PartialEq)]
pub struct ControllerFirmwares {
    pub bootloader: ControllerVersion,
    pub backup_fw: ControllerVersion,
    pub main_fw: ControllerVersion,
}

#[derive(Debug, PartialEq)]
pub struct PdVersions {
    pub controller01: ControllerFirmwares,
    pub controller23: ControllerFirmwares,
}

/// Same as PdVersions but only the main FW
pub struct MainPdVersions {
    pub controller01: ControllerVersion,
    pub controller23: ControllerVersion,
}

pub fn get_pd_controller_versions() -> EcResult<PdVersions> {
    Ok(PdVersions {
        controller01: PdController::new(PdPort::Left01).get_fw_versions()?,
        controller23: PdController::new(PdPort::Right23).get_fw_versions()?,
    })
}

//fn parse_metadata(buffer: &[u8; 256]) -> Option<(u32, u32)> {
fn parse_metadata_cyacd(buffer: &[u8]) -> Option<(u32, u32)> {
    let buffer = &buffer[METADATA_OFFSET..];
    let metadata_len = std::mem::size_of::<CyAcdMetadata>();
    let metadata: CyAcdMetadata =
        unsafe { std::ptr::read(buffer[0..metadata_len].as_ptr() as *const _) };
    //println!("Metadata: {:?}", metadata);
    if metadata.metadata_valid == METADATA_MAGIC {
        Some((1 + metadata.boot_last_row as u32, metadata.fw_size))
    } else {
        None
    }
}
