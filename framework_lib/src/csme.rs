#[cfg(feature = "linux")]
use std::fs;
#[cfg(feature = "linux")]
use std::io;
#[cfg(feature = "linux")]
use std::path::Path;

pub struct CsmeInfo {
    pub enabled: bool,
    pub ver_platform: u32,
    pub ver_major: u32,
    pub ver_minor: u32,
    pub ver_hotfix: u32,
    pub ver_buildno: u32,
}

pub fn format_csme_ver(info: &CsmeInfo) -> String {
    format!(
        "{}:{}.{}.{}.{}",
        info.ver_platform, info.ver_major, info.ver_minor, info.ver_hotfix, info.ver_buildno
    )
}

#[cfg(feature = "linux")]
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
                // TODO: Make sure invalid cases are handled and not silently ignored
                let enabled = matches!(dev_state.as_str(), "ENABLED");

                let fw_vers = fs::read_to_string(path.join("fw_ver"))?;
                // Kernel gives us multiple \n separated lines
                let fw_vers: Vec<&str> = fw_vers.lines().collect();
                // TODO: I don't understand why the kernel gives me 4 versions.
                // Make sure my assumption that all versios are the same holds tru.
                assert!(fw_vers.iter().all(|&item| item == fw_vers[0]));
                let fw_ver: &str = fw_vers[0];
                // Parse the CSME version
                // Example: 0:16.0.15.1810
                let sections: Vec<&str> = fw_ver.split(':').collect();
                let first = sections[0].parse::<u32>().expect("Unexpected value");
                let right: Vec<&str> = sections[1].split('.').collect();
                let second = right[0].parse::<u32>().expect("Unexpected value");
                let third = right[1].parse::<u32>().expect("Unexpected value");
                let fourth = right[2].parse::<u32>().expect("Unexpected value");
                let fifth = right[3].trim().parse::<u32>().expect("Unexpected value");

                csme_info = Some(CsmeInfo {
                    enabled,
                    ver_platform: first,
                    ver_major: second,
                    ver_minor: third,
                    ver_hotfix: fourth,
                    ver_buildno: fifth,
                });
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
