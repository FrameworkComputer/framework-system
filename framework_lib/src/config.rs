use serde::Deserialize;

use crate::util;

#[derive(Debug, Deserialize)]
struct Config {
    platform: Option<Platform>,
}

#[derive(Debug, Deserialize)]
struct Platform {
    has_mec: bool,
    pd_addrs: Vec<u16>,
    pd_ports: Vec<u8>,
}

const CONFIG_FILE: &str = "framework_tool_config.toml";

#[cfg(feature = "uefi")]
fn read_config_file() -> String {
    crate::uefi::fs::shell_read_file(CONFIG_FILE)
}
#[cfg(not(feature = "uefi"))]
fn read_config_file() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.push(CONFIG_FILE);

    if let Ok(str) = std::fs::read_to_string(path) {
        str
    } else {
        path = CONFIG_FILE.into();
        std::fs::read_to_string(path).unwrap()
    }
}

pub fn load_config() -> Option<util::Platform> {
    let toml_str = read_config_file();

    let decoded: Config = toml::from_str(&toml_str).unwrap();
    println!("{:?}", decoded);

    let decoded = decoded.platform.unwrap();
    let first_pd = (decoded.pd_addrs[0], decoded.pd_ports[0]);
    let second_pd = if decoded.pd_addrs.is_empty() || decoded.pd_ports.is_empty() {
        first_pd
    } else {
        (decoded.pd_addrs[1], decoded.pd_ports[1])
    };

    Some(util::Platform::GenericFramework(
        (first_pd.0, second_pd.0),
        (first_pd.1, second_pd.1),
        decoded.has_mec,
    ))
}
