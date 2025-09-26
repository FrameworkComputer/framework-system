//! Module to build a portable commandline tool
//!
//! Can be easily re-used from any OS or UEFI shell.
//! We have implemented both in the `framework_tool` and `framework_uefi` crates.

use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use guid_create::{CGuid, GUID};
use log::Level;
use num_traits::FromPrimitive;

#[cfg(not(feature = "uefi"))]
pub mod clap_std;
#[cfg(feature = "uefi")]
pub mod uefi;

#[cfg(not(feature = "uefi"))]
use std::fs;
#[cfg(not(feature = "uefi"))]
use std::io::prelude::*;

#[cfg(feature = "rusb")]
use crate::audio_card::check_synaptics_fw_version;
use crate::built_info;
#[cfg(feature = "rusb")]
use crate::camera::check_camera_version;
use crate::capsule;
use crate::capsule_content::{
    find_bios_version, find_ec_in_bios_cap, find_pd_in_bios_cap, find_retimer_version,
};
use crate::ccgx::device::{FwMode, PdController, PdPort};
#[cfg(feature = "hidapi")]
use crate::ccgx::hid::{check_ccg_fw_version, find_devices, DP_CARD_PID, HDMI_CARD_PID};
use crate::ccgx::{self, MainPdVersions, PdVersions, SiliconId::*};
use crate::chromium_ec;
use crate::chromium_ec::commands::DeckStateMode;
use crate::chromium_ec::commands::FpLedBrightnessLevel;
use crate::chromium_ec::commands::RebootEcCmd;
use crate::chromium_ec::commands::RgbS;
use crate::chromium_ec::commands::TabletModeOverride;
use crate::chromium_ec::EcResponseStatus;
use crate::chromium_ec::{print_err, EcFlashType};
use crate::chromium_ec::{EcError, EcResult};
#[cfg(target_os = "linux")]
use crate::csme;
use crate::ec_binary;
use crate::esrt;
#[cfg(feature = "rusb")]
use crate::inputmodule::check_inputmodule_version;
use crate::os_specific;
use crate::parade_retimer;
use crate::power;
use crate::smbios;
use crate::smbios::ConfigDigit0;
use crate::smbios::{dmidecode_string_val, get_smbios, is_framework};
#[cfg(feature = "hidapi")]
use crate::touchpad::print_touchpad_fw_ver;
#[cfg(feature = "hidapi")]
use crate::touchscreen;
#[cfg(feature = "uefi")]
use crate::uefi::enable_page_break;
#[cfg(feature = "rusb")]
use crate::usbhub::check_usbhub_version;
use crate::util::{self, Config, Platform, PlatformFamily};
#[cfg(feature = "hidapi")]
use hidapi::HidApi;
use sha2::{Digest, Sha256, Sha384, Sha512};
//use smbioslib::*;
use smbioslib::{DefinedStruct, SMBiosInformation};

use crate::chromium_ec::{CrosEc, CrosEcDriverType, HardwareDeviceType};

#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

#[cfg_attr(not(feature = "uefi"), derive(clap::ValueEnum))]
#[derive(Clone, Debug, PartialEq)]
pub enum TabletModeArg {
    Auto,
    Tablet,
    Laptop,
}

#[cfg_attr(not(feature = "uefi"), derive(clap::ValueEnum))]
#[derive(Clone, Debug, PartialEq)]
pub enum ConsoleArg {
    Recent,
    Follow,
}

#[cfg_attr(not(feature = "uefi"), derive(clap::ValueEnum))]
#[derive(Clone, Debug, PartialEq)]
pub enum RebootEcArg {
    Reboot,
    JumpRo,
    JumpRw,
    CancelJump,
    DisableJump,
}

#[cfg_attr(not(feature = "uefi"), derive(clap::ValueEnum))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FpBrightnessArg {
    High,
    Medium,
    Low,
    UltraLow,
    Auto,
}
impl From<FpBrightnessArg> for FpLedBrightnessLevel {
    fn from(w: FpBrightnessArg) -> FpLedBrightnessLevel {
        match w {
            FpBrightnessArg::High => FpLedBrightnessLevel::High,
            FpBrightnessArg::Medium => FpLedBrightnessLevel::Medium,
            FpBrightnessArg::Low => FpLedBrightnessLevel::Low,
            FpBrightnessArg::UltraLow => FpLedBrightnessLevel::UltraLow,
            FpBrightnessArg::Auto => FpLedBrightnessLevel::Auto,
        }
    }
}

#[cfg_attr(not(feature = "uefi"), derive(clap::ValueEnum))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputDeckModeArg {
    Auto,
    Off,
    On,
}
impl From<InputDeckModeArg> for DeckStateMode {
    fn from(w: InputDeckModeArg) -> DeckStateMode {
        match w {
            InputDeckModeArg::Auto => DeckStateMode::Required,
            InputDeckModeArg::Off => DeckStateMode::ForceOff,
            InputDeckModeArg::On => DeckStateMode::ForceOn,
        }
    }
}

#[derive(Debug)]
pub struct LogLevel(log::LevelFilter);

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel(log::LevelFilter::Error)
    }
}

/// Shadows `clap_std::ClapCli` with extras for UEFI
///
/// The UEFI commandline currently doesn't use clap, so we need to shadow the struct.
/// Also it has extra options.
#[derive(Debug, Default)]
pub struct Cli {
    pub verbosity: LogLevel,
    pub versions: bool,
    pub version: bool,
    pub features: bool,
    pub esrt: bool,
    pub device: Option<HardwareDeviceType>,
    pub compare_version: Option<String>,
    pub power: bool,
    pub thermal: bool,
    pub sensors: bool,
    pub fansetduty: Option<(Option<u32>, u32)>,
    pub fansetrpm: Option<(Option<u32>, u32)>,
    pub autofanctrl: bool,
    pub pdports: bool,
    pub privacy: bool,
    pub pd_info: bool,
    pub pd_reset: Option<u8>,
    pub pd_disable: Option<u8>,
    pub pd_enable: Option<u8>,
    pub dp_hdmi_info: bool,
    pub dp_hdmi_update: Option<String>,
    pub audio_card_info: bool,
    pub pd_bin: Option<String>,
    pub ec_bin: Option<String>,
    pub capsule: Option<String>,
    pub dump: Option<String>,
    pub h2o_capsule: Option<String>,
    pub dump_ec_flash: Option<String>,
    pub flash_ec: Option<String>,
    pub flash_ro_ec: Option<String>,
    pub flash_rw_ec: Option<String>,
    pub driver: Option<CrosEcDriverType>,
    pub test: bool,
    pub dry_run: bool,
    pub force: bool,
    pub intrusion: bool,
    pub inputdeck: bool,
    pub inputdeck_mode: Option<InputDeckModeArg>,
    pub expansion_bay: bool,
    pub charge_limit: Option<Option<u8>>,
    pub charge_current_limit: Option<(u32, Option<u32>)>,
    pub charge_rate_limit: Option<(f32, Option<f32>)>,
    pub get_gpio: Option<Option<String>>,
    pub fp_led_level: Option<Option<FpBrightnessArg>>,
    pub fp_brightness: Option<Option<u8>>,
    pub kblight: Option<Option<u8>>,
    pub remap_key: Option<(u8, u8, u16)>,
    pub rgbkbd: Vec<u64>,
    pub ps2_enable: Option<bool>,
    pub tablet_mode: Option<TabletModeArg>,
    pub touchscreen_enable: Option<bool>,
    pub stylus_battery: bool,
    pub console: Option<ConsoleArg>,
    pub reboot_ec: Option<RebootEcArg>,
    pub ec_hib_delay: Option<Option<u32>>,
    pub hash: Option<String>,
    pub pd_addrs: Option<(u16, u16, u16)>,
    pub pd_ports: Option<(u8, u8, u8)>,
    pub help: bool,
    pub info: bool,
    pub flash_gpu_descriptor: Option<(u8, String)>,
    pub flash_gpu_descriptor_file: Option<String>,
    pub dump_gpu_descriptor_file: Option<String>,
    // UEFI only
    pub allupdate: bool,
    pub paginate: bool,
    // TODO: This is not actually implemented yet
    pub raw_command: Vec<String>,
}

pub fn parse(args: &[String]) -> Cli {
    #[cfg(feature = "uefi")]
    let cli = uefi::parse(args);
    #[cfg(not(feature = "uefi"))]
    let cli = clap_std::parse(args);

    if cfg!(feature = "readonly") {
        // Initialize a new Cli with no arguments
        // Set all arguments that are readonly/safe
        // We explicitly only cope the safe ones so that if we add new arguments in the future,
        // which might be unsafe, we can't forget to exclude them from the safe set.
        // TODO: Instead of silently ignoring blocked command, we should remind the user
        Cli {
            verbosity: cli.verbosity,
            versions: cli.versions,
            version: cli.version,
            features: cli.features,
            esrt: cli.esrt,
            device: cli.device,
            compare_version: cli.compare_version,
            power: cli.power,
            thermal: cli.thermal,
            sensors: cli.sensors,
            // fansetduty
            // fansetrpm
            // autofanctrl
            pdports: cli.pdports,
            privacy: cli.privacy,
            pd_info: cli.version,
            // pd_reset
            // pd_disable
            // pd_enable
            dp_hdmi_info: cli.dp_hdmi_info,
            // dp_hdmi_update
            audio_card_info: cli.audio_card_info,
            pd_bin: cli.pd_bin,
            ec_bin: cli.ec_bin,
            capsule: cli.capsule,
            dump: cli.dump,
            h2o_capsule: cli.h2o_capsule,
            // dump_ec_flash
            // flash_ec
            // flash_ro_ec
            // flash_rw_ec
            driver: cli.driver,
            test: cli.test,
            dry_run: cli.dry_run,
            // force
            intrusion: cli.intrusion,
            inputdeck: cli.inputdeck,
            inputdeck_mode: cli.inputdeck_mode,
            expansion_bay: cli.expansion_bay,
            // charge_limit
            // charge_current_limit
            // charge_rate_limit
            get_gpio: cli.get_gpio,
            fp_led_level: cli.fp_led_level,
            fp_brightness: cli.fp_brightness,
            kblight: cli.kblight,
            remap_key: cli.remap_key,
            rgbkbd: cli.rgbkbd,
            ps2_enable: cli.ps2_enable,
            // tablet_mode
            // touchscreen_enable
            stylus_battery: cli.stylus_battery,
            console: cli.console,
            reboot_ec: cli.reboot_ec,
            // ec_hib_delay
            hash: cli.hash,
            pd_addrs: cli.pd_addrs,
            pd_ports: cli.pd_ports,
            help: cli.help,
            info: cli.info,
            // flash_gpu_descriptor
            // flash_gpu_descriptor_file
            // allupdate
            paginate: cli.paginate,
            // raw_command
            ..Default::default()
        }
    } else {
        cli
    }
}

fn print_single_pd_details(pd: &PdController) {
    if let Ok(si) = pd.get_silicon_id() {
        println!("  Silicon ID:     0x{:X}", si);
    } else {
        println!("  Failed to read Silicon ID/Family");
        return;
    }
    if let Ok((mode, frs)) = pd.get_device_info() {
        println!("  Mode:           {:?}", mode);
        println!("  Flash Row Size: {} B", frs);
    } else {
        println!("  Failed to device info");
    }
    if let Ok(port_mask) = pd.get_port_status() {
        let ports = match port_mask {
            1 => "0",
            2 => "1",
            3 => "0, 1",
            _ => "None",
        };
        println!("  Ports Enabled:  {}", ports);
    } else {
        println!("  Ports Enabled:  Unknown");
    }
    pd.print_fw_info();
}

fn print_pd_details(ec: &CrosEc) {
    if !is_framework() {
        println!("Only supported on Framework systems");
        return;
    }
    let pd_01 = PdController::new(PdPort::Right01, ec.clone());
    let pd_23 = PdController::new(PdPort::Left23, ec.clone());
    let pd_back = PdController::new(PdPort::Back, ec.clone());

    println!("Right / Ports 01");
    print_single_pd_details(&pd_01);
    println!("Left / Ports 23");
    print_single_pd_details(&pd_23);
    println!("Back");
    print_single_pd_details(&pd_back);
}

#[cfg(feature = "hidapi")]
const NOT_SET: &str = "NOT SET";

#[cfg(feature = "rusb")]
fn print_audio_card_details() {
    check_synaptics_fw_version();
}

#[cfg(feature = "hidapi")]
fn print_dp_hdmi_details(verbose: bool) {
    match HidApi::new() {
        Ok(api) => {
            for dev_info in find_devices(&api, &[HDMI_CARD_PID, DP_CARD_PID], None) {
                let vid = dev_info.vendor_id();
                let pid = dev_info.product_id();

                let device = dev_info.open_device(&api).unwrap();
                if let Some(name) = ccgx::hid::device_name(vid, pid) {
                    println!("{}", name);
                }

                // On Windows this value is "Control Interface", probably hijacked by the kernel driver
                debug!(
                    "  Product String:  {}",
                    dev_info.product_string().unwrap_or(NOT_SET)
                );

                debug!(
                    "  Serial Number:        {}",
                    dev_info.serial_number().unwrap_or(NOT_SET)
                );
                check_ccg_fw_version(&device, verbose);
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
        }
    };
}

fn print_tool_version() {
    let q = "?".to_string();
    println!("Tool Version Information");
    println!("  Version:     {}", built_info::PKG_VERSION);
    println!("  Built At:    {}", built_info::BUILT_TIME_UTC);
    println!(
        "  Git Commit:  {}",
        built_info::GIT_COMMIT_HASH.unwrap_or(&q)
    );
    println!(
        "  Git Dirty:   {}",
        built_info::GIT_DIRTY
            .map(|x| x.to_string())
            .unwrap_or(q.clone())
    );

    if log_enabled!(Level::Info) {
        println!(
            "  Built on CI: {:?}",
            built_info::CI_PLATFORM.unwrap_or("None")
        );
        println!(
            "  Git ref:     {:?}",
            built_info::GIT_HEAD_REF.unwrap_or(&q)
        );
        println!("  rustc Ver:   {}", built_info::RUSTC_VERSION);
        println!("  Features     {:?}", built_info::FEATURES);
        println!("  DEBUG:       {}", built_info::DEBUG);
        println!("  Target OS:   {}", built_info::CFG_OS);
    }
}

// TODO: Check if HDMI card is same
#[cfg(feature = "hidapi")]
fn flash_dp_hdmi_card(pd_bin_path: &str) {
    let data = match fs::read(pd_bin_path) {
        Ok(data) => Some(data),
        // TODO: Perhaps a more user-friendly error
        Err(e) => {
            println!("Error {:?}", e);
            None
        }
    };
    if let Some(data) = data {
        // TODO: Check if exists, otherwise err
        //ccgx::hid::find_device().unwrap();
        ccgx::hid::flash_firmware(&data);
    } else {
        error!("Failed to open firmware file");
    }
}

fn active_mode(mode: &FwMode, reference: FwMode) -> &'static str {
    if mode == &reference {
        " (Active)"
    } else {
        ""
    }
}

#[cfg(feature = "hidapi")]
fn print_stylus_battery_level() {
    loop {
        if let Some(level) = touchscreen::get_battery_level() {
            println!("Stylus Battery Strength: {}%", level);
            return;
        } else {
            debug!("Stylus Battery Strength: Unknown");
        }
    }
}

fn print_versions(ec: &CrosEc) {
    println!("Tool Version:     {}", built_info::PKG_VERSION);
    println!("OS Version:       {}", os_specific::get_os_version());
    println!("Mainboard Hardware");
    if let Some(ver) = smbios::get_product_name() {
        println!("  Type:           {}", ver);
    } else {
        println!("  Type:           Unknown");
    }
    if let Some(ver) = smbios::get_baseboard_version() {
        println!("  Revision:       {:?}", ver);
    } else {
        println!("  Revision:       Unknown");
    }
    println!("UEFI BIOS");
    if let Some(smbios) = get_smbios() {
        let bios_entries = smbios.collect::<SMBiosInformation>();
        let bios = bios_entries.first().unwrap();
        println!("  Version:        {}", bios.version());
        println!("  Release Date:   {}", bios.release_date());
    } else {
        println!("  Version:        Unknown");
    }

    println!("EC Firmware");
    let ver = print_err(ec.version_info()).unwrap_or_else(|| "UNKNOWN".to_string());
    println!("  Build version:  {}", ver);

    if let Some((ro, rw, curr)) = ec.flash_version() {
        if ro != rw || log_enabled!(Level::Info) {
            println!("  RO Version:     {}", ro);
            println!("  RW Version:     {}", rw);
        }
        print!("  Current image:  ");
        if curr == chromium_ec::EcCurrentImage::RO {
            println!("RO");
        } else if curr == chromium_ec::EcCurrentImage::RW {
            println!("RW");
        } else {
            println!("Unknown");
        }
    } else {
        println!("  RO Version:     Unknown");
        println!("  RW Version:     Unknown");
        println!("  Current image:  Unknown");
    }

    println!("PD Controllers");
    let ccgx_pd_vers = ccgx::get_pd_controller_versions(ec);
    if let Ok(PdVersions::RightLeft((right, left))) = ccgx_pd_vers {
        if let Some(Platform::IntelGen11) = smbios::get_platform() {
            if right.main_fw.base != right.backup_fw.base {
                println!("  Right (01)");
                println!(
                    "    Main:           {}{}",
                    right.main_fw.base,
                    active_mode(&right.active_fw, FwMode::MainFw)
                );
                println!(
                    "    Backup:         {}{}",
                    right.backup_fw.base,
                    active_mode(&right.active_fw, FwMode::BackupFw)
                );
            } else {
                println!(
                    "  Right (01):       {} ({:?})",
                    right.main_fw.base, right.active_fw
                );
            }
        } else if right.main_fw.app != right.backup_fw.app {
            println!(
                "    Main:           {}{}",
                right.main_fw.app,
                active_mode(&right.active_fw, FwMode::MainFw)
            );
            println!(
                "    Backup:         {}{}",
                right.backup_fw.app,
                active_mode(&right.active_fw, FwMode::BackupFw)
            );
        } else {
            println!(
                "  Right (01):       {} ({:?})",
                right.main_fw.app, right.active_fw
            );
        }
        if let Some(Platform::IntelGen11) = smbios::get_platform() {
            if left.main_fw.base != left.backup_fw.base {
                println!("  Left  (23)");
                println!(
                    "    Main:           {}{}",
                    left.main_fw.base,
                    active_mode(&left.active_fw, FwMode::MainFw)
                );
                println!(
                    "    Backup:         {}{}",
                    left.backup_fw.base,
                    active_mode(&left.active_fw, FwMode::BackupFw)
                );
            } else {
                println!(
                    "  Left  (23):       {} ({:?})",
                    left.main_fw.base, left.active_fw
                );
            }
        } else if left.main_fw.app != left.backup_fw.app {
            println!("  Left  (23)");
            println!(
                "    Main:           {}{}",
                left.main_fw.app,
                active_mode(&left.active_fw, FwMode::MainFw)
            );
            println!(
                "    Backup:         {}{}",
                left.backup_fw.app,
                active_mode(&left.active_fw, FwMode::BackupFw)
            );
        } else {
            println!(
                "  Left  (23):       {} ({:?})",
                left.main_fw.app, left.active_fw
            );
        }
    } else if let Ok(PdVersions::Many(versions)) = ccgx_pd_vers {
        for (i, version) in versions.into_iter().enumerate() {
            if version.main_fw.app != version.backup_fw.app {
                println!("  PD {}", 1);
                println!(
                    "    Main:           {}{}",
                    version.main_fw.app,
                    active_mode(&version.active_fw, FwMode::MainFw)
                );
                println!(
                    "    Backup:         {}{}",
                    version.backup_fw.app,
                    active_mode(&version.active_fw, FwMode::BackupFw)
                );
            } else {
                println!(
                    "  PD {}:            {} ({:?})",
                    i, version.main_fw.app, version.active_fw
                );
            }
        }
    } else if let Ok(PdVersions::Single(pd)) = ccgx_pd_vers {
        if pd.main_fw.app != pd.backup_fw.app {
            println!(
                "    Main:         {}{}",
                pd.main_fw.app,
                active_mode(&pd.active_fw, FwMode::MainFw)
            );
            println!(
                "    Backup:       {}{}",
                pd.backup_fw.app,
                active_mode(&pd.active_fw, FwMode::BackupFw)
            );
        } else {
            println!("  Version:        {} ({:?})", pd.main_fw.app, pd.active_fw);
        }
    } else if let Ok(pd_versions) = power::read_pd_version(ec) {
        // As fallback try to get it from the EC. But not all EC versions have this command
        debug!("  Fallback to PD Host command");
        match pd_versions {
            MainPdVersions::RightLeft((controller01, controller23)) => {
                if let Some(Platform::IntelGen11) = smbios::get_platform() {
                    println!("  Right (01):     {}", controller01.base);
                    println!("  Left  (23):     {}", controller23.base);
                } else {
                    println!("  Right (01):     {}", controller01.app);
                    println!("  Left  (23):     {}", controller23.app);
                }
            }
            MainPdVersions::Single(version) => {
                println!("  Version:        {}", version.app);
            }
            MainPdVersions::Many(versions) => {
                for (i, version) in versions.into_iter().enumerate() {
                    println!("  PD {}:          {}", i, version.app);
                }
            }
        }
    } else {
        println!("  Unknown")
    }

    let has_retimer = matches!(
        smbios::get_platform(),
        Some(Platform::IntelGen11)
            | Some(Platform::IntelGen12)
            | Some(Platform::IntelGen13)
            | Some(Platform::IntelCoreUltra1)
    );
    let mut left_retimer: Option<u32> = None;
    let mut right_retimer: Option<u32> = None;
    if let Some(esrt) = esrt::get_esrt() {
        for entry in &esrt.entries {
            match GUID::from(entry.fw_class) {
                esrt::TGL_RETIMER01_GUID
                | esrt::ADL_RETIMER01_GUID
                | esrt::RPL_RETIMER01_GUID
                | esrt::MTL_RETIMER01_GUID => {
                    right_retimer = Some(entry.fw_version);
                }
                esrt::TGL_RETIMER23_GUID
                | esrt::ADL_RETIMER23_GUID
                | esrt::RPL_RETIMER23_GUID
                | esrt::MTL_RETIMER23_GUID => {
                    left_retimer = Some(entry.fw_version);
                }
                _ => {}
            }
        }
    }
    if has_retimer {
        println!("Intel Retimers");
        if let Some(fw_version) = left_retimer {
            println!("  Left:           0x{:X} ({})", fw_version, fw_version);
        }
        if let Some(fw_version) = right_retimer {
            println!("  Right:          0x{:X} ({})", fw_version, fw_version);
        }
        if left_retimer.is_none() && right_retimer.is_none() {
            // This means there's a bug, we should've found one but didn't
            println!("  Unknown");
        }
    }
    match parade_retimer::get_version(ec) {
        // Does not exist
        Ok(None) => {}
        Ok(Some(ver)) => {
            println!("Parade Retimers");
            println!(
                "  dGPU:           {:X}.{:X}.{:X}.{:X}",
                ver[0], ver[1], ver[2], ver[3]
            );
        }
        _err => {
            println!("Parade Retimers");
            println!("  Unknown");
        }
    }

    #[cfg(target_os = "linux")]
    if smbios::get_platform().and_then(Platform::which_cpu_vendor) != Some(util::CpuVendor::Amd) {
        println!("CSME");
        if let Ok(csme) = csme::csme_from_sysfs() {
            info!("  Enabled:          {}", csme.enabled);
            println!("  Firmware Version: {}", csme.main_ver);
            if csme.main_ver != csme.recovery_ver || csme.main_ver != csme.fitc_ver {
                println!("  Recovery Ver:     {}", csme.recovery_ver);
                println!("  Original Ver:     {}", csme.fitc_ver);
            }
        } else {
            println!("  Unknown");
        }
    }
    #[cfg(feature = "rusb")]
    let _ignore_err = check_camera_version();

    #[cfg(feature = "rusb")]
    let _ignore_err = check_usbhub_version();

    #[cfg(feature = "rusb")]
    let _ignore_err = check_inputmodule_version();

    #[cfg(feature = "hidapi")]
    let _ignore_err = print_touchpad_fw_ver();

    #[cfg(feature = "hidapi")]
    if let Some(Platform::Framework12IntelGen13) = smbios::get_platform() {
        let _ignore_err = touchscreen::print_fw_ver();
    }
    #[cfg(feature = "hidapi")]
    print_dp_hdmi_details(false);
}

fn print_esrt() {
    if let Some(esrt) = esrt::get_esrt() {
        esrt::print_esrt(&esrt);
    } else {
        println!("Could not find and parse ESRT table.");
    }
}

fn flash_ec(ec: &CrosEc, ec_bin_path: &str, flash_type: EcFlashType, dry_run: bool) {
    #[cfg(feature = "uefi")]
    let data = crate::uefi::fs::shell_read_file(ec_bin_path);
    #[cfg(not(feature = "uefi"))]
    let data: Option<Vec<u8>> = {
        match fs::read(ec_bin_path) {
            Ok(data) => Some(data),
            // TODO: Perhaps a more user-friendly error
            Err(e) => {
                println!("Error {:?}", e);
                None
            }
        }
    };

    if let Some(data) = data {
        println!("File");
        println!("  Size:       {:>20} B", data.len());
        println!("  Size:       {:>20} KB", data.len() / 1024);
        if let Err(err) = ec.reflash(&data, flash_type, dry_run) {
            println!("Error: {:?}", err);
        } else {
            println!("Success!");
        }
    }
}

fn dump_ec_flash(ec: &CrosEc, dump_path: &str) {
    let flash_bin = ec.get_entire_ec_flash().unwrap();

    #[cfg(not(feature = "uefi"))]
    {
        let mut file = fs::File::create(dump_path).unwrap();
        file.write_all(&flash_bin).unwrap();
    }
    #[cfg(feature = "uefi")]
    {
        let ret = crate::uefi::fs::shell_write_file(dump_path, &flash_bin);
        if ret.is_err() {
            println!("Failed to dump EC FW image.");
        }
    }
}

fn dump_dgpu_eeprom(ec: &CrosEc, dump_path: &str) {
    let flash_bin = ec.read_gpu_descriptor().unwrap();

    #[cfg(not(feature = "uefi"))]
    {
        let mut file = fs::File::create(dump_path).unwrap();
        file.write_all(&flash_bin).unwrap();
    }
    #[cfg(feature = "uefi")]
    {
        let ret = crate::uefi::fs::shell_write_file(dump_path, &flash_bin);
        if ret.is_err() {
            println!("Failed to dump EC FW image.");
        }
    }
    println!("Wrote {} bytes to {}", flash_bin.len(), dump_path);
}

fn compare_version(device: Option<HardwareDeviceType>, version: String, ec: &CrosEc) -> i32 {
    println!("Target Version {:?}", version);

    if let Some(smbios) = get_smbios() {
        let bios_entries = smbios.collect::<SMBiosInformation>();
        let bios = bios_entries.first().unwrap();

        if device == Some(HardwareDeviceType::BIOS) {
            println!("Comparing BIOS version {:?}", bios.version().to_string());
            if version.to_uppercase() == bios.version().to_string().to_uppercase() {
                return 0;
            } else {
                return 1;
            }
        }
    }

    match device {
        Some(HardwareDeviceType::EC) => {
            let ver = print_err(ec.version_info()).unwrap_or_else(|| "UNKNOWN".to_string());
            println!("Comparing EC version {:?}", ver);

            if ver.contains(&version) {
                return 0;
            } else {
                return 1;
            }
        }
        Some(HardwareDeviceType::PD0) => {
            if let Ok(PdVersions::RightLeft((pd01, _pd23))) = ccgx::get_pd_controller_versions(ec) {
                let ver = pd01.active_fw_ver();
                println!("Comparing PD0 version {:?}", ver);

                if ver.contains(&version) {
                    return 0;
                } else {
                    return 1;
                }
            }
        }
        Some(HardwareDeviceType::PD1) => {
            if let Ok(PdVersions::RightLeft((_pd01, pd23))) = ccgx::get_pd_controller_versions(ec) {
                let ver = pd23.active_fw_ver();
                println!("Comparing PD1 version {:?}", ver);

                if ver.contains(&version) {
                    return 0;
                } else {
                    return 1;
                }
            }
        }
        Some(HardwareDeviceType::AcLeft) => {
            if let Ok((_right, left)) = power::is_charging(ec) {
                let ver = format!("{}", left as i32);
                println!("Comparing AcLeft {:?}", ver);
                if ver == version {
                    return 0;
                } else {
                    return 1;
                }
            } else {
                error!("Failed to get charging information");
                // Not charging is the safe default
                return 1;
            }
        }
        Some(HardwareDeviceType::AcRight) => {
            if let Ok((right, _left)) = power::is_charging(ec) {
                let ver = format!("{}", right as i32);
                println!("Comparing AcRight {:?}", ver);
                if ver == version {
                    return 0;
                } else {
                    return 1;
                }
            } else {
                error!("Failed to get charging information");
                // Not charging is the safe default
                return 1;
            }
        }
        _ => {}
    }

    if let Some(esrt) = esrt::get_esrt() {
        for entry in &esrt.entries {
            match GUID::from(entry.fw_class) {
                esrt::TGL_RETIMER01_GUID | esrt::ADL_RETIMER01_GUID | esrt::RPL_RETIMER01_GUID => {
                    if device == Some(HardwareDeviceType::RTM01) {
                        println!("Comparing RTM01 version {:?}", entry.fw_version.to_string());

                        if entry.fw_version.to_string().contains(&version) {
                            return 0;
                        }
                    }
                }
                esrt::TGL_RETIMER23_GUID | esrt::ADL_RETIMER23_GUID | esrt::RPL_RETIMER23_GUID => {
                    if device == Some(HardwareDeviceType::RTM23) {
                        println!("Comparing RTM23 version {:?}", entry.fw_version.to_string());
                        if entry.fw_version.to_string().contains(&version) {
                            return 0;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    1
}

pub fn run_with_args(args: &Cli, _allupdate: bool) -> i32 {
    #[cfg(feature = "uefi")]
    {
        log::set_max_level(args.verbosity.0);
    }
    #[cfg(not(feature = "uefi"))]
    {
        // TOOD: Should probably have a custom env variable?
        // let env = Env::default()
        //     .filter("FRAMEWORK_COMPUTER_LOG")
        //     .write_style("FRAMEWORK_COMPUTER_LOG_STYLE");

        let level = args.verbosity.0.as_str();
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level))
            .format_target(false)
            .format_timestamp(None)
            .init();
    }

    // Must be run before any application code to set the config
    if args.pd_addrs.is_some() && args.pd_ports.is_some() {
        let platform = Platform::GenericFramework(args.pd_addrs.unwrap(), args.pd_ports.unwrap());
        Config::set(platform);
    }

    let ec = if let Some(driver) = args.driver {
        if let Some(driver) = CrosEc::with(driver) {
            driver
        } else {
            println!("Selected driver {:?} not available.", driver);
            return 1;
        }
    } else {
        CrosEc::new()
    };

    #[cfg(feature = "uefi")]
    if args.paginate {
        enable_page_break();
    }

    if args.help {
        // Only print with uefi feature here because without clap will already
        // have printed the help by itself.
        #[cfg(feature = "uefi")]
        print_help(_allupdate);
        return 2;
    } else if args.versions {
        print_versions(&ec);
    } else if args.version {
        print_tool_version();
    } else if args.features {
        print_err(ec.get_features());
    } else if args.esrt {
        print_esrt();
    } else if let Some(compare_version_ver) = &args.compare_version {
        let compare_ret = compare_version(args.device, compare_version_ver.to_string(), &ec);
        println!("Comparison Result:  {}", compare_ret);
        return compare_ret;
    } else if args.intrusion {
        println!("Chassis status:");
        if let Some(status) = print_err(ec.get_intrusion_status()) {
            println!(
                "  Coin cell ever removed:   {}",
                status.coin_cell_ever_removed
            );
            println!("  Chassis currently open:   {}", status.currently_open);
            println!("  Chassis ever opened:      {}", status.ever_opened);
            println!("  Chassis opened:           {} times", status.total_opened);
            println!(
                "  Chassis opened while off: {} times",
                status.vtr_open_count
            );
        } else {
            println!("  Unable to tell");
        }
    } else if args.inputdeck {
        let res = match smbios::get_platform().and_then(Platform::which_family) {
            Some(PlatformFamily::Framework12) => ec.print_fw12_inputdeck_status(),
            Some(PlatformFamily::Framework13) => ec.print_fw13_inputdeck_status(),
            Some(PlatformFamily::Framework16) => ec.print_fw16_inputdeck_status(),
            // If we don't know which platform it is, we can use some heuristics
            _ => {
                // Only Framework 16 has this GPIO
                if ec.get_gpio("sleep_l").is_ok() {
                    ec.print_fw16_inputdeck_status()
                } else {
                    println!("  Unable to tell");
                    Ok(())
                }
            }
        };
        print_err(res);
    } else if let Some(mode) = &args.inputdeck_mode {
        println!("Set mode to: {:?}", mode);
        ec.set_input_deck_mode((*mode).into()).unwrap();
    } else if args.expansion_bay {
        if let Err(err) = ec.check_bay_status() {
            error!("{:?}", err);
        }
        if let Ok(header) = ec.read_gpu_desc_header() {
            println!("  Expansion Bay EEPROM");
            println!(
                "    Valid:       {:?}",
                header.magic == [0x32, 0xAC, 0x00, 0x00]
            );
            println!("    HW Version:  {}.{}", { header.hardware_version }, {
                header.hardware_revision
            });
            if log_enabled!(Level::Info) {
                println!("    Hdr Length   {} B", { header.length });
                println!("    Desc Ver:    {}.{}", { header.desc_ver_major }, {
                    header.desc_ver_minor
                });
                println!("    Serialnumber:{:X?}", { header.serial });
                println!("    Desc Length: {} B", { header.descriptor_length });
                println!("    Desc CRC:    {:X}", { header.descriptor_crc32 });
                println!("    Hdr CRC:     {:X}", { header.crc32 });
            }
        }
    } else if let Some(maybe_limit) = args.charge_limit {
        print_err(handle_charge_limit(&ec, maybe_limit));
    } else if let Some((limit, soc)) = args.charge_current_limit {
        print_err(ec.set_charge_current_limit(limit, soc));
    } else if let Some((limit, soc)) = args.charge_rate_limit {
        print_err(ec.set_charge_rate_limit(limit, soc));
    } else if let Some(gpio_name) = &args.get_gpio {
        if let Some(gpio_name) = gpio_name {
            print!("GPIO {}: ", gpio_name);
            if let Ok(value) = ec.get_gpio(gpio_name) {
                println!("{:?}", value);
            } else {
                println!("Not found");
            }
        } else {
            print_err(ec.get_all_gpios());
        }
    } else if let Some(maybe_led_level) = &args.fp_led_level {
        print_err(handle_fp_led_level(&ec, *maybe_led_level));
    } else if let Some(maybe_brightness) = &args.fp_brightness {
        print_err(handle_fp_brightness(&ec, *maybe_brightness));
    } else if let Some(Some(kblight)) = args.kblight {
        assert!(kblight <= 100);
        ec.set_keyboard_backlight(kblight);
    } else if let Some(None) = args.kblight {
        print!("Keyboard backlight: ");
        if let Some(percentage) = print_err(ec.get_keyboard_backlight()) {
            println!("{}%", percentage);
        } else {
            println!("Unable to tell");
        }
    } else if let Some((row, col, scanset)) = args.remap_key {
        print_err(ec.remap_key(row, col, scanset));
    } else if !args.rgbkbd.is_empty() {
        if args.rgbkbd.len() < 2 {
            println!(
                "Must provide at least 2 arguments. Provided only: {}",
                args.rgbkbd.len()
            );
        } else {
            let start_key = args.rgbkbd[0] as u8;
            let colors = args.rgbkbd[1..].iter().map(|color| RgbS {
                r: ((color & 0x00FF0000) >> 16) as u8,
                g: ((color & 0x0000FF00) >> 8) as u8,
                b: (color & 0x000000FF) as u8,
            });
            ec.rgbkbd_set_color(start_key, colors.collect()).unwrap();
        }
    } else if let Some(enable) = args.ps2_enable {
        print_err(ec.ps2_emulation_enable(enable));
    } else if let Some(tablet_arg) = &args.tablet_mode {
        let mode = match tablet_arg {
            TabletModeArg::Auto => TabletModeOverride::Default,
            TabletModeArg::Tablet => TabletModeOverride::ForceTablet,
            TabletModeArg::Laptop => TabletModeOverride::ForceClamshell,
        };
        ec.set_tablet_mode(mode);
    } else if let Some(_enable) = &args.touchscreen_enable {
        #[cfg(feature = "hidapi")]
        if touchscreen::enable_touch(*_enable).is_none() {
            error!("Failed to enable/disable touch");
        }
    } else if args.stylus_battery {
        #[cfg(feature = "hidapi")]
        print_stylus_battery_level();
        #[cfg(not(feature = "hidapi"))]
        error!("Not build with hidapi feature");
    } else if let Some(console_arg) = &args.console {
        match console_arg {
            ConsoleArg::Follow => {
                // Ignore result because we only finish when it crashes
                let _res = ec.console_read();
            }
            ConsoleArg::Recent => match ec.console_read_one() {
                Ok(output) => println!("{}", output),
                Err(err) => println!("Failed to read console: {:?}", err),
            },
        }
    } else if let Some(reboot_arg) = &args.reboot_ec {
        match reboot_arg {
            RebootEcArg::Reboot => match ec.reboot_ec(RebootEcCmd::ColdReboot) {
                Ok(_) => {}
                Err(err) => println!("Failed: {:?}", err),
            },
            RebootEcArg::JumpRo => match ec.jump_ro() {
                Ok(_) => {}
                Err(err) => println!("Failed: {:?}", err),
            },
            RebootEcArg::JumpRw => match ec.jump_rw() {
                Ok(_) => {}
                Err(err) => println!("Failed: {:?}", err),
            },
            RebootEcArg::CancelJump => match ec.cancel_jump() {
                Ok(_) => {}
                Err(err) => println!("Failed: {:?}", err),
            },
            RebootEcArg::DisableJump => match ec.disable_jump() {
                Ok(_) => {}
                Err(err) => println!("Failed: {:?}", err),
            },
        }
    } else if let Some(delay) = &args.ec_hib_delay {
        if let Some(delay) = delay {
            print_err(ec.set_ec_hib_delay(*delay));
        }
        print_err(ec.get_ec_hib_delay());
    } else if args.test {
        println!("Self-Test");
        let result = selftest(&ec);
        if result.is_none() {
            println!("FAILED!!");
            return 1;
        }
    } else if args.power {
        return power::get_and_print_power_info(&ec);
    } else if args.thermal {
        power::print_thermal(&ec);
    } else if args.sensors {
        power::print_sensors(&ec);
    } else if let Some((fan, percent)) = args.fansetduty {
        print_err(ec.fan_set_duty(fan, percent));
    } else if let Some((fan, rpm)) = args.fansetrpm {
        print_err(ec.fan_set_rpm(fan, rpm));
    } else if args.autofanctrl {
        print_err(ec.autofanctrl(None));
    } else if args.pdports {
        power::get_and_print_pd_info(&ec);
    } else if args.info {
        smbios_info();
    } else if args.pd_info {
        print_pd_details(&ec);
    } else if let Some(pd) = args.pd_reset {
        println!("Resetting PD {}...", pd);
        print_err(match pd {
            0 => PdController::new(PdPort::Right01, ec.clone()).reset_device(),
            1 => PdController::new(PdPort::Left23, ec.clone()).reset_device(),
            2 => PdController::new(PdPort::Back, ec.clone()).reset_device(),
            _ => {
                error!("PD {} does not exist", pd);
                Ok(())
            }
        });
    } else if let Some(pd) = args.pd_disable {
        println!("Disabling PD {}...", pd);
        print_err(match pd {
            0 => PdController::new(PdPort::Right01, ec.clone()).enable_ports(false),
            1 => PdController::new(PdPort::Left23, ec.clone()).enable_ports(false),
            2 => PdController::new(PdPort::Back, ec.clone()).enable_ports(false),
            _ => {
                error!("PD {} does not exist", pd);
                Ok(())
            }
        });
    } else if let Some(pd) = args.pd_enable {
        println!("Enabling PD {}...", pd);
        print_err(match pd {
            0 => PdController::new(PdPort::Right01, ec.clone()).enable_ports(true),
            1 => PdController::new(PdPort::Left23, ec.clone()).enable_ports(true),
            2 => PdController::new(PdPort::Back, ec.clone()).enable_ports(true),
            _ => {
                error!("PD {} does not exist", pd);
                Ok(())
            }
        });
    } else if args.dp_hdmi_info {
        #[cfg(feature = "hidapi")]
        print_dp_hdmi_details(true);
    } else if let Some(pd_bin_path) = &args.dp_hdmi_update {
        #[cfg(feature = "hidapi")]
        flash_dp_hdmi_card(pd_bin_path);
        #[cfg(not(feature = "hidapi"))]
        let _ = pd_bin_path;
    } else if args.audio_card_info {
        #[cfg(feature = "rusb")]
        print_audio_card_details();
    } else if args.privacy {
        if let Some((mic, cam)) = print_err(ec.get_privacy_info()) {
            println!("Privacy Slider (Black = Device Connected; Red = Device Disconnected)");
            println!(
                "  Microphone:  {}",
                if mic { "Connected" } else { "Disconnected" }
            );
            println!(
                "  Camera:      {}",
                if cam { "Connected" } else { "Disconnected" }
            );
        } else {
            println!("Not all EC versions support this comand.")
        };
    // TODO:
    //} else if arg == "-raw-command" {
    //    raw_command(&args[1..]);
    } else if let Some(pd_bin_path) = &args.pd_bin {
        #[cfg(feature = "uefi")]
        let data: Option<Vec<u8>> = crate::uefi::fs::shell_read_file(pd_bin_path);
        #[cfg(not(feature = "uefi"))]
        let data = match fs::read(pd_bin_path) {
            Ok(data) => Some(data),
            // TODO: Perhaps a more user-friendly error
            Err(e) => {
                println!("Error {:?}", e);
                None
            }
        };

        if let Some(data) = data {
            println!("File");
            println!("  Size:       {:>20} B", data.len());
            println!("  Size:       {:>20} KB", data.len() / 1024);
            analyze_ccgx_pd_fw(&data);
        }
    } else if let Some(ec_bin_path) = &args.ec_bin {
        #[cfg(feature = "uefi")]
        let data: Option<Vec<u8>> = crate::uefi::fs::shell_read_file(ec_bin_path);
        #[cfg(not(feature = "uefi"))]
        let data = match fs::read(ec_bin_path) {
            Ok(data) => Some(data),
            // TODO: Perhaps a more user-friendly error
            Err(e) => {
                println!("Error {:?}", e);
                None
            }
        };

        if let Some(data) = data {
            println!("File");
            println!("  Size:       {:>20} B", data.len());
            println!("  Size:       {:>20} KB", data.len() / 1024);
            analyze_ec_fw(&data);
        }
    } else if let Some(capsule_path) = &args.capsule {
        #[cfg(feature = "uefi")]
        let data: Option<Vec<u8>> = crate::uefi::fs::shell_read_file(capsule_path);
        #[cfg(not(feature = "uefi"))]
        let data = match fs::read(capsule_path) {
            Ok(data) => Some(data),
            // TODO: Perhaps a more user-friendly error
            Err(e) => {
                println!("Error {:?}", e);
                None
            }
        };

        if let Some(data) = data {
            println!("File");
            println!("  Size:       {:>20} B", data.len());
            println!("  Size:       {:>20} KB", data.len() / 1024);
            if let Some(header) = analyze_capsule(&data) {
                if header.capsule_guid == CGuid::from(esrt::WINUX_GUID) {
                    let ux_header = capsule::parse_ux_header(&data);
                    if let Some(dump_path) = &args.dump {
                        // TODO: Better error handling, rather than just panicking
                        capsule::dump_winux_image(&data, &ux_header, dump_path);
                    }
                }
            } else {
                println!("Capsule is invalid.");
            }
        }
    } else if let Some(capsule_path) = &args.h2o_capsule {
        #[cfg(feature = "uefi")]
        let data = crate::uefi::fs::shell_read_file(capsule_path);
        #[cfg(not(feature = "uefi"))]
        let data = match fs::read(capsule_path) {
            Ok(data) => Some(data),
            // TODO: Perhaps a more user-friendly error
            Err(e) => {
                println!("Error {:?}", e);
                None
            }
        };

        if let Some(data) = data {
            println!("File");
            println!("  Size:       {:>20} B", data.len());
            println!("  Size:       {:>20} KB", data.len() / 1024);
            if let Some(cap) = find_bios_version(&data) {
                println!("  BIOS Platform:{:>18}", cap.platform);
                println!("  BIOS Version: {:>18}", cap.version);
            }
            if let Some(ec_bin) = find_ec_in_bios_cap(&data) {
                debug!("Found EC binary in BIOS capsule");
                analyze_ec_fw(ec_bin);
            } else {
                debug!("Didn't find EC binary in BIOS capsule");
            }
            if let Some(pd_bin) = find_pd_in_bios_cap(&data) {
                debug!("Found PD binary in BIOS capsule");
                analyze_ccgx_pd_fw(pd_bin);
            } else {
                debug!("Didn't find PD binary in BIOS capsule");
            }
        }
    } else if let Some(dump_path) = &args.dump_ec_flash {
        println!("Dumping to {}", dump_path);
        // TODO: Should have progress indicator
        dump_ec_flash(&ec, dump_path);
    } else if let Some(ec_bin_path) = &args.flash_ec {
        if args.force {
            flash_ec(&ec, ec_bin_path, EcFlashType::Full, args.dry_run);
        } else {
            error!("Flashing EC RO region is unsafe. Use --flash-ec-rw instead");
        }
    } else if let Some(ec_bin_path) = &args.flash_ro_ec {
        if args.force {
            flash_ec(&ec, ec_bin_path, EcFlashType::Ro, args.dry_run);
        } else {
            error!("Flashing EC RO region is unsafe. Use --flash-ec-rw instead");
        }
    } else if let Some(ec_bin_path) = &args.flash_rw_ec {
        flash_ec(&ec, ec_bin_path, EcFlashType::Rw, args.dry_run);
    } else if let Some(hash_file) = &args.hash {
        println!("Hashing file: {}", hash_file);
        #[cfg(feature = "uefi")]
        let data = crate::uefi::fs::shell_read_file(hash_file);
        #[cfg(not(feature = "uefi"))]
        let data = match fs::read(hash_file) {
            Ok(data) => Some(data),
            // TODO: Perhaps a more user-friendly error
            Err(e) => {
                println!("Error {:?}", e);
                None
            }
        };
        if let Some(data) = data {
            println!("File");
            println!("  Size:       {:>20} B", data.len());
            println!("  Size:       {:>20} KB", data.len() / 1024);
            hash(&data);
        }
    } else if let Some(gpu_descriptor) = &args.flash_gpu_descriptor {
        let res = ec.set_gpu_serial(gpu_descriptor.0, gpu_descriptor.1.to_ascii_uppercase());
        match res {
            Ok(1) => println!("GPU Descriptor successfully written"),
            Ok(x) => println!("GPU Descriptor write failed with status code:  {}", x),
            Err(err) => println!("GPU Descriptor write failed with error:  {:?}", err),
        }
    } else if let Some(gpu_descriptor_file) = &args.flash_gpu_descriptor_file {
        if matches!(
            smbios::get_family(),
            Some(PlatformFamily::Framework16) | None
        ) {
            #[cfg(feature = "uefi")]
            let data: Option<Vec<u8>> = crate::uefi::fs::shell_read_file(gpu_descriptor_file);
            #[cfg(not(feature = "uefi"))]
            let data = match fs::read(gpu_descriptor_file) {
                Ok(data) => Some(data),
                // TODO: Perhaps a more user-friendly error
                Err(e) => {
                    println!("Error {:?}", e);
                    None
                }
            };
            if let Some(data) = data {
                println!("File");
                println!("  Size:       {:>20} B", data.len());
                println!("  Size:       {:>20} KB", data.len() / 1024);
                let res = ec.set_gpu_descriptor(&data, args.dry_run);
                match res {
                    Ok(()) => println!("GPU Descriptor successfully written"),
                    Err(err) => println!("GPU Descriptor write failed with error:  {:?}", err),
                }
            }
        } else {
            println!("Unsupported on this platform");
        }
    } else if let Some(dump_path) = &args.dump_gpu_descriptor_file {
        println!("Dumping to {}", dump_path);
        dump_dgpu_eeprom(&ec, dump_path);
    }

    0
}

// Only on UEFI. Clap prints this by itself
#[cfg(feature = "uefi")]
fn print_help(updater: bool) {
    println!(
        r#"Swiss army knife for Framework laptops

Usage: framework_tool [OPTIONS]

Options:
  -v, --verbose...           More output per occurrence
  -q, --quiet...             Less output per occurrence
      --versions             List current firmware versions
      --version              Show tool version information (Add -vv for more detailed information)
      --features             Show features support by the firmware
      --esrt                 Display the UEFI ESRT table
      --device <DEVICE>      Device used to compare firmware version [possible values: bios, ec, pd0, pd1, rtm01, rtm23]
      --compare-version      Version string used to match firmware version (use with --device)
      --power                Show current power status (battery and AC)
      --thermal              Print thermal information (Temperatures and Fan speed)
      --sensors              Print sensor information (ALS, G-Sensor)
      --fansetduty           Set fan duty cycle (0-100%)
      --fansetrpm            Set fan RPM (limited by EC fan table max RPM)
      --autofanctrl          Turn on automatic fan speed control
      --pdports              Show information about USB-C PD ports
      --info                 Show info from SMBIOS (Only on UEFI)
      --pd-info              Show details about the PD controllers
      --privacy              Show privacy switch statuses (camera and microphone)
      --pd-bin <PD_BIN>      Parse versions from PD firmware binary file
      --ec-bin <EC_BIN>      Parse versions from EC firmware binary file
      --capsule <CAPSULE>    Parse UEFI Capsule information from binary file
      --dump <DUMP>          Dump extracted UX capsule bitmap image to a file
      --h2o-capsule <H2O_CAPSULE>      Parse UEFI Capsule information from binary file
      --dump-ec-flash <DUMP_EC_FLASH>  Dump EC flash contents
      --flash-ec <FLASH_EC>            Flash EC with new firmware from file
      --flash-ro-ec <FLASH_EC>         Flash EC with new firmware from file
      --flash-rw-ec <FLASH_EC>         Flash EC with new firmware from file
      --reboot-ec            Control EC RO/RW jump [possible values: reboot, jump-ro, jump-rw, cancel-jump, disable-jump]
      --intrusion            Show status of intrusion switch
      --inputdeck            Show status of the input deck
      --inputdeck-mode       Set input deck power mode [possible values: auto, off, on] (Framework 16 only)
      --expansion-bay        Show status of the expansion bay (Framework 16 only)
      --charge-limit [<VAL>] Get or set battery charge limit (Percentage number as arg, e.g. '100')
      --charge-current-limit [<VAL>] Get or set battery current charge limit (Percentage number as arg, e.g. '100')
      --get-gpio <GET_GPIO>  Get GPIO value by name or all, if no name provided
      --fp-led-level [<VAL>] Get or set fingerprint LED brightness level [possible values: high, medium, low]
      --fp-brightness [<VAL>]Get or set fingerprint LED brightness percentage
      --kblight [<KBLIGHT>]  Set keyboard backlight percentage or get, if no value provided
      --console <CONSOLE>    Get EC console, choose whether recent or to follow the output [possible values: recent, follow]
      --hash <HASH>          Hash a file of arbitrary data
      --flash-gpu-descriptor <MAGIC> <18 DIGIT SN> Overwrite the GPU bay descriptor SN and type.
      --flash-gpu-descriptor-file <DESCRIPTOR_FILE> Write the GPU bay descriptor with a descriptor file.
  -f, --force                Force execution of an unsafe command - may render your hardware unbootable!
      --dry-run              Simulate execution of a command (e.g. --flash-ec)
  -t, --test                 Run self-test to check if interaction with EC is possible
  -h, --help                 Print help information
  -b                         Print output one screen at a time
    "#
    );
    if updater {
        println!(
            r#"
        --allupdate   - Run procedure to update everything (Involves some manual steps)
    "#
        );
    }
    // TODO: Not supported yet
    //println!(
    //    r#"
    //    --raw-command - Send a raw command to the EC
    //                    Example: raw-command 0x3E14
    //"#
    //);
}

/// Useful to hash update files to check integrity
fn hash(data: &[u8]) {
    let mut sha256_hasher = Sha256::new();
    let mut sha384_hasher = Sha384::new();
    let mut sha512_hasher = Sha512::new();

    sha256_hasher.update(data);
    sha384_hasher.update(data);
    sha512_hasher.update(data);

    let sha256 = &sha256_hasher.finalize()[..];
    let sha384 = &sha384_hasher.finalize()[..];
    let sha512 = &sha512_hasher.finalize()[..];

    println!("Hashes");
    print!("  SHA256:  ");
    util::print_buffer(sha256);
    print!("  SHA384:  ");
    util::print_buffer(sha384);
    print!("  SHA512:  ");
    util::print_buffer(sha512);
}

fn selftest(ec: &CrosEc) -> Option<()> {
    if let Some(platform) = smbios::get_platform() {
        println!("  SMBIOS Platform:     {:?}", platform);
    } else {
        println!("  SMBIOS Platform:     Unknown");
        println!();
        println!("Specify custom platform parameters with --pd-ports --pd-addrs");
        return None;
    };

    println!("  Dump EC memory region");
    if let Some(mem) = ec.dump_mem_region() {
        util::print_multiline_buffer(&mem, 0);
    } else {
        println!("    Failed to read EC memory region");
        return None;
    }

    println!("  Checking EC memory mapped magic bytes");
    print_err(ec.check_mem_magic())?;
    println!("  Verified that Framework EC is present!");

    println!("  Reading EC Build Version");
    print_err(ec.version_info())?;

    print!("  Reading EC Flash by EC");
    ec.flash_version()?;
    println!(" - OK");

    println!("  Reading EC Flash directly - See below");
    ec.test_ec_flash_read().ok()?;

    print!("  Getting power info from EC");
    power::power_info(ec)?;
    println!(" - OK");

    println!("  Getting AC info from EC");
    // All our laptops have at least 4 PD ports so far
    if power::get_pd_info(ec, 4).iter().any(|x| x.is_err()) {
        println!("    Failed to get PD Info from EC");
        return None;
    }

    print!("Reading PD Version from EC");
    if let Err(err) = power::read_pd_version(ec) {
        // TGL does not have this command, so we have to ignore it
        if err != EcError::Response(EcResponseStatus::InvalidCommand) {
            println!();
            println!("Err: {:?}", err);
        } else {
            println!(" - Skipped");
        }
    } else {
        println!(" - OK");
    }

    let pd_01 = PdController::new(PdPort::Right01, ec.clone());
    let pd_23 = PdController::new(PdPort::Left23, ec.clone());
    print!("  Getting PD01 info through I2C tunnel");
    print_err(pd_01.get_silicon_id())?;
    print_err(pd_01.get_device_info())?;
    print_err(pd_01.get_fw_versions())?;
    println!(" - OK");
    print!("  Getting PD23 info through I2C tunnel");
    print_err(pd_23.get_silicon_id())?;
    print_err(pd_23.get_device_info())?;
    print_err(pd_23.get_fw_versions())?;
    println!(" - OK");

    Some(())
}

fn smbios_info() {
    println!("Summary");
    println!("  Is Framework: {}", is_framework());
    if let Some(platform) = smbios::get_platform() {
        println!("  Platform:     {:?}", platform);
    } else {
        println!("  Platform:     Unknown",);
    }

    let smbios = get_smbios();
    if smbios.is_none() {
        error!("Failed to find SMBIOS");
        return;
    }
    for undefined_struct in smbios.unwrap().iter() {
        match undefined_struct.defined_struct() {
            DefinedStruct::Information(data) => {
                println!("BIOS Information");
                if let Some(vendor) = dmidecode_string_val(&data.vendor()) {
                    println!("  Vendor:       {}", vendor);
                }
                if let Some(version) = dmidecode_string_val(&data.version()) {
                    println!("  Version:      {}", version);
                }
                if let Some(release_date) = dmidecode_string_val(&data.release_date()) {
                    println!("  Release Date: {}", release_date);
                }
            }
            DefinedStruct::SystemInformation(data) => {
                println!("System Information");
                if let Some(version) = dmidecode_string_val(&data.version()) {
                    // Assumes it's ASCII, which is guaranteed by SMBIOS
                    let config_digit0 = &version[0..1];
                    let config_digit0 = u8::from_str_radix(config_digit0, 16);
                    if let Ok(version_config) =
                        config_digit0.map(<ConfigDigit0 as FromPrimitive>::from_u8)
                    {
                        println!("  Version:      {:?} ({})", version_config, version);
                    } else {
                        println!("  Version:      '{}'", version);
                    }
                }
                if let Some(manufacturer) = dmidecode_string_val(&data.manufacturer()) {
                    println!("  Manufacturer: {}", manufacturer);
                }
                if let Some(product_name) = dmidecode_string_val(&data.product_name()) {
                    println!("  Product Name: {}", product_name);
                }
                if let Some(wake_up_type) = data.wakeup_type() {
                    println!("  Wake-Up-Type: {:?}", wake_up_type.value);
                }
                if let Some(sku_number) = dmidecode_string_val(&data.sku_number()) {
                    println!("  SKU Number:   {}", sku_number);
                }
                if let Some(sn) = dmidecode_string_val(&data.serial_number()) {
                    println!("  Serial Number:{}", sn);
                }
                if let Some(family) = dmidecode_string_val(&data.family()) {
                    println!("  Family:       {}", family);
                }
            }
            DefinedStruct::SystemChassisInformation(data) => {
                println!("System Chassis Information");
                if let Some(chassis) = data.chassis_type() {
                    println!("  Type:         {}", chassis);
                }
            }
            DefinedStruct::BaseBoardInformation(data) => {
                println!("BaseBoard Information");
                if let Some(version) = dmidecode_string_val(&data.version()) {
                    // Assumes it's ASCII, which is guaranteed by SMBIOS
                    let config_digit0 = &version[0..1];
                    let config_digit0 = u8::from_str_radix(config_digit0, 16);
                    if let Ok(version_config) =
                        config_digit0.map(<ConfigDigit0 as FromPrimitive>::from_u8)
                    {
                        println!("  Version:      {:?} ({})", version_config, version);
                    } else {
                        println!("  Version:      '{}'", version);
                    }
                }
                if let Some(manufacturer) = dmidecode_string_val(&data.manufacturer()) {
                    println!("  Manufacturer: {}", manufacturer);
                }
                if let Some(product_name) = dmidecode_string_val(&data.product()) {
                    println!("  Product:      {}", product_name);
                }
                if let Some(sn) = dmidecode_string_val(&data.serial_number()) {
                    println!("  Serial Number:{}", sn);
                }
            }
            _ => {}
        }
    }
}

fn analyze_ccgx_pd_fw(data: &[u8]) {
    if let Some(versions) = ccgx::binary::read_versions(data, Ccg3) {
        println!("Detected CCG3 firmware");
        println!("FW 1");
        ccgx::binary::print_fw(&versions.backup_fw);

        println!("FW 2");
        ccgx::binary::print_fw(&versions.main_fw);
    } else if let Some(versions) = ccgx::binary::read_versions(data, Ccg8) {
        println!("Detected CCG8 firmware");
        println!("FW 1");
        ccgx::binary::print_fw(&versions.backup_fw);

        println!("FW 2");
        ccgx::binary::print_fw(&versions.main_fw);
    } else if let Some(versions) = ccgx::binary::read_versions(data, Ccg5) {
        println!("Detected CCG5 firmware");
        println!("FW 1");
        ccgx::binary::print_fw(&versions.backup_fw);

        println!("FW 2");
        ccgx::binary::print_fw(&versions.main_fw);
    } else if let Some(versions) = ccgx::binary::read_versions(data, Ccg6) {
        println!("Detected CCG6 firmware");
        println!("FW 1 (Backup)");
        ccgx::binary::print_fw(&versions.backup_fw);

        println!("FW 2 (Main)");
        ccgx::binary::print_fw(&versions.main_fw);
    } else {
        println!("Failed to read PD versions")
    }
}

pub fn analyze_ec_fw(data: &[u8]) {
    // Readonly firmware
    if let Some(ver) = ec_binary::read_ec_version(data, true) {
        ec_binary::print_ec_version(&ver, true);
    } else {
        println!("Failed to read EC version")
    }
    // Readwrite firmware
    if let Some(ver) = ec_binary::read_ec_version(data, false) {
        ec_binary::print_ec_version(&ver, false);
    } else {
        println!("Failed to read EC version")
    }
}

pub fn analyze_capsule(data: &[u8]) -> Option<capsule::EfiCapsuleHeader> {
    let header = capsule::parse_capsule_header(data)?;
    capsule::print_capsule_header(&header);

    match GUID::from(header.capsule_guid) {
        esrt::TGL_BIOS_GUID => {
            println!("  Type:         Framework TGL Insyde BIOS");
        }
        esrt::ADL_BIOS_GUID => {
            println!("  Type:         Framework ADL Insyde BIOS");
        }
        esrt::RPL_BIOS_GUID => {
            println!("  Type:         Framework RPL Insyde BIOS");
        }
        esrt::TGL_RETIMER01_GUID => {
            println!("  Type:    Framework TGL Retimer01 (Right)");
        }
        esrt::TGL_RETIMER23_GUID => {
            println!("  Type:   Framework TGL Retimer23 (Left)");
        }
        esrt::ADL_RETIMER01_GUID => {
            println!("  Type:    Framework ADL Retimer01 (Right)");
        }
        esrt::ADL_RETIMER23_GUID => {
            println!("  Type:   Framework ADL Retimer23 (Left)");
        }
        esrt::RPL_RETIMER01_GUID => {
            println!("  Type:    Framework RPL Retimer01 (Right)");
        }
        esrt::RPL_RETIMER23_GUID => {
            println!("  Type:   Framework RPL Retimer23 (Left)");
        }
        esrt::WINUX_GUID => {
            println!("  Type:            Windows UX capsule");
            let ux_header = capsule::parse_ux_header(data);
            capsule::print_ux_header(&ux_header);
        }
        _ => {
            println!("  Type:                      Unknown");
        }
    }

    match esrt::match_guid_kind(&header.capsule_guid) {
        esrt::FrameworkGuidKind::TglRetimer01
        | esrt::FrameworkGuidKind::TglRetimer23
        | esrt::FrameworkGuidKind::AdlRetimer01
        | esrt::FrameworkGuidKind::AdlRetimer23
        | esrt::FrameworkGuidKind::RplRetimer01
        | esrt::FrameworkGuidKind::RplRetimer23 => {
            if let Some(ver) = find_retimer_version(data) {
                println!("  Version:      {:>18?}", ver);
            }
        }
        _ => {}
    }

    Some(header)
}

fn handle_charge_limit(ec: &CrosEc, maybe_limit: Option<u8>) -> EcResult<()> {
    let (cur_min, _cur_max) = ec.get_charge_limit()?;
    if let Some(limit) = maybe_limit {
        // Prevent setting unreasonable limits
        if limit < 25 {
            return Err(EcError::DeviceError(
                "Not recommended to set charge limit below 25%".to_string(),
            ));
        } else if limit > 100 {
            return Err(EcError::DeviceError(
                "Charge limit cannot be set above 100%".to_string(),
            ));
        }
        ec.set_charge_limit(cur_min, limit)?;
    }

    let (min, max) = ec.get_charge_limit()?;
    println!("Minimum {}%, Maximum {}%", min, max);

    Ok(())
}

fn handle_fp_led_level(ec: &CrosEc, maybe_led_level: Option<FpBrightnessArg>) -> EcResult<()> {
    if let Some(led_level) = maybe_led_level {
        ec.set_fp_led_level(led_level.into())?;
    }

    let (brightness, level) = ec.get_fp_led_level()?;
    // TODO: Rename to power button
    println!("Fingerprint LED Brightness");
    if let Some(level) = level {
        println!("  Requested:  {:?}", level);
    }
    println!("  Brightness: {}%", brightness);

    Ok(())
}

fn handle_fp_brightness(ec: &CrosEc, maybe_brightness: Option<u8>) -> EcResult<()> {
    if let Some(brightness) = maybe_brightness {
        ec.set_fp_led_percentage(brightness)?;
    }

    let (brightness, level) = ec.get_fp_led_level()?;
    // TODO: Rename to power button
    println!("Fingerprint LED Brightness");
    if let Some(level) = level {
        println!("  Requested:  {:?}", level);
    }
    println!("  Brightness: {}%", brightness);

    Ok(())
}
