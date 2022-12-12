#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;
use std::fmt;

use crate::chromium_ec::EcResult;

use self::device::{PdController, PdPort};

pub mod binary;
pub mod device;

#[derive(Debug)]
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
#[derive(Debug)]
pub enum Application {
    Notebook,
    Monitor,
    Invalid,
}

#[derive(Debug)]
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

// TODO: Consider bootloader and both firmwares
pub struct ControllerVersion {
    pub base: BaseVersion,
    pub app: AppVersion,
}

pub struct PdVersions {
    pub controller01: ControllerVersion,
    pub controller23: ControllerVersion,
}

pub fn get_pd_controller_versions() -> EcResult<PdVersions> {
    Ok(PdVersions {
        controller01: PdController::new(PdPort::Left01).get_fw_versions()?,
        controller23: PdController::new(PdPort::Right23).get_fw_versions()?,
    })
}
