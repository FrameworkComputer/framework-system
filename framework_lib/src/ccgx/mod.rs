//! Interact with Infineon (formerly Cypress) PD controllers (their firmware binaries) in the CCGx series

#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;
use num_derive::FromPrimitive;
use std::fmt;

use crate::chromium_ec::EcResult;

use self::device::{PdController, PdPort};

pub mod binary;
pub mod device;

const FW1_METADATA_ROW: u32 = 0x1FE;
const FW2_METADATA_ROW_CCG5: u32 = 0x1FF;
const FW2_METADATA_ROW_CCG6: u32 = 0x1FD;
const LAST_BOOTLOADER_ROW: usize = 0x05;
const FW_SIZE_OFFSET: usize = 0x09;
const METADATA_OFFSET: usize = 0xC0; // TODO: Is this 0x40 on ADL?
const METADATA_MAGIC_OFFSET: usize = 0x16;
const METADATA_MAGIC_1: u8 = 0x59;
const METADATA_MAGIC_2: u8 = 0x43;

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
            "{}.{}.{}.{}",
            self.major, self.minor, self.patch, self.build_number
        )
    }
}
impl From<&[u8]> for BaseVersion {
    fn from(data: &[u8]) -> Self {
        Self {
            build_number: ((data[1] as u16) << 8) + (data[0] as u16),
            patch: data[2],
            major: (data[3] & 0xF0) >> 4,
            minor: data[3] & 0x0F,
        }
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
        write!(f, "{}.{}.{}", self.major, self.minor, self.circuit)
    }
}

impl From<&[u8]> for AppVersion {
    fn from(data: &[u8]) -> Self {
        let application = if data[0] == 0x62 && data[1] == 0x6e {
            Application::Notebook // ASCII "nb"
        } else if data[0] == 0x64 && data[1] == 0x6d {
            Application::Monitor // ASCII "md"
        } else {
            debug_assert!(false, "Couldn't parse application");
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
fn parse_metadata(buffer: &[u8]) -> Option<(u32, u32)> {
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
