//! Get CSME information from the running system
//!
//! Currently only works on Linux (from sysfs).

use core::fmt;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::io;
#[cfg(target_os = "linux")]
use std::path::Path;

pub struct CsmeInfo {
    /// Whether the CSME is currently enabled or not
    pub enabled: bool,
    /// Currently running CSME firmware version
    pub main_ver: CsmeVersion,
    pub recovery_ver: CsmeVersion,
    pub fitc_ver: CsmeVersion,
}
/// CSME Version
///
/// Example: 0:16.0.15.1810
#[derive(Debug, PartialEq, Eq)]
pub struct CsmeVersion {
    pub platform: u32,
    pub major: u32,
    pub minor: u32,
    pub hotfix: u32,
    pub buildno: u32,
}

impl From<&str> for CsmeVersion {
    fn from(fw_ver: &str) -> Self {
        // Parse the CSME version
        // Example: 0:16.0.15.1810
        let mut sections = fw_ver.split(':');

        let left = sections
            .next()
            .unwrap()
            .parse::<u32>()
            .expect("Unexpected value");
        let mut right = sections.next().unwrap().split('.');

        let second = right
            .next()
            .unwrap()
            .parse::<u32>()
            .expect("Unexpected value");
        let third = right
            .next()
            .unwrap()
            .parse::<u32>()
            .expect("Unexpected value");
        let fourth = right
            .next()
            .unwrap()
            .parse::<u32>()
            .expect("Unexpected value");
        let fifth = right
            .next()
            .unwrap()
            .trim()
            .parse::<u32>()
            .expect("Unexpected value");

        CsmeVersion {
            platform: left,
            major: second,
            minor: third,
            hotfix: fourth,
            buildno: fifth,
        }
    }
}

impl fmt::Display for CsmeVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}.{}.{}.{}",
            self.platform, self.major, self.minor, self.hotfix, self.buildno
        )
    }
}

#[cfg(target_os = "linux")]
pub fn csme_from_sysfs() -> io::Result<CsmeInfo> {
    let dir = Path::new("/sys/class/mei");
    let mut csme_info: Option<CsmeInfo> = None;
    if dir.is_dir() {
        for csmeme_entry in fs::read_dir(dir)? {
            // Can currently only handle one ME. Not sure when there would be multiple?
            assert!(csme_info.is_none());

            let csmeme_entry = csmeme_entry?;
            let path = csmeme_entry.path();
            if path.is_dir() {
                let dev_state = fs::read_to_string(path.join("dev_state"))?;
                // Can be one of INITIALIZING, INIT_CLIENTS, ENABLED, RESETTING, DISABLED,
                // POWER_DOWN, POWER_UP
                // See linux kernel at: Documentation/ABI/testing/sysfs-class-mei
                let enabled = matches!(dev_state.as_str(), "ENABLED");

                // Kernel gives us multiple \n separated lines in a file
                let fw_vers = fs::read_to_string(path.join("fw_ver"))?;
                let fw_vers = fw_vers.lines();

                let mut infos = fw_vers.map(CsmeVersion::from);
                let main_ver = infos.next().unwrap();
                let recovery_ver = infos.next().unwrap();
                let fitc_ver = infos.next().unwrap();
                // Make sure there are three and no more
                assert_eq!(infos.next(), None);

                csme_info = Some(CsmeInfo {
                    enabled,
                    main_ver,
                    recovery_ver,
                    fitc_ver,
                })
            }
        }
    }
    if let Some(csme_info) = csme_info {
        Ok(csme_info)
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Failed to get CSME info from sysfs",
        ))
    }
}
