const CROS_EC_IMAGE_DATA_COOKIE1: u32 = 0xce778899;
const CROS_EC_IMAGE_DATA_COOKIE2: u32 = 0xceaabbdd;
const PD_VERSION_OFFSET: usize = 0x1158;

use core::prelude::v1::derive;
#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
struct _ImageVersionData {
    cookie1: u32,
    version: [u8; 32],
    size: u32,
    rollback_version: u32,
    cookie2: u32,
}
#[derive(Debug)]
pub struct ImageVersionData {
    pub version: String,
    pub platform: String,
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub commit: String,
    pub size: u32,
    pub rollback_version: u32,
}

pub fn print_ec_version(ver: ImageVersionData) {
    println!("EC");
    println!("  Version:    {:>20}", ver.version);
    println!("  RollbackVer:{:>20}", ver.rollback_version);
    println!("  Platform:   {:>20}", ver.platform);
    let version = format!("{}.{}.{}", ver.major, ver.minor, ver.patch);
    println!("  Version:    {:>20}", version);
    println!("  Commit:     {:>20}", ver.commit);
    println!("  Size:       {:>20} B", ver.size);
    println!("  Size:       {:>20} KB", ver.size / 1024);
}

pub fn read_ec_version(data: &[u8]) -> Option<ImageVersionData> {
    let v: _ImageVersionData = unsafe {
        std::ptr::read(data[PD_VERSION_OFFSET..].as_ptr() as *const _)
    };
    if v.cookie1 != CROS_EC_IMAGE_DATA_COOKIE1 {
        println!("Failed to find Cookie 1");
        return None;
    }
    if v.cookie2 != CROS_EC_IMAGE_DATA_COOKIE2 {
        println!("Failed to find Cookie 2");
        return None;
    }

    let version = std::str::from_utf8(&v.version).ok()?.trim_end_matches(char::from(0));
    // Example: hx30_v0.0.1-7a61a89
    let re = regex::Regex::new(r"([a-z0-9]+)_v([0-9])\.([0-9])\.([0-9])-([0-9a-f]+)").unwrap();
    let caps = re.captures(version).unwrap();
    let platform = caps.get(1)?.as_str().to_string();
    let major = caps.get(2)?.as_str().parse::<u32>().ok()?;
    let minor = caps.get(3)?.as_str().parse::<u32>().ok()?;
    let patch = caps.get(4)?.as_str().parse::<u32>().ok()?;
    let commit = caps.get(5)?.as_str().to_string();

    Some(ImageVersionData {
        version: version.to_string(),
        size: v.size,
        rollback_version: v.rollback_version,
        platform,
        major,
        minor,
        patch,
        commit,
    })
}
