//! Module to build a portable commandline tool
//!
//! Can be easily re-used from any OS or UEFI shell.
//! We have implemented both in the `framework_tool` and `framework_uefi` crates.

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use log::Level;

#[cfg(not(feature = "uefi"))]
pub mod clap_std;
#[cfg(feature = "uefi")]
pub mod uefi;

#[cfg(not(feature = "uefi"))]
use std::fs;

#[cfg(not(feature = "uefi"))]
use crate::audio_card::check_synaptics_fw_version;
use crate::built_info;
use crate::capsule;
use crate::capsule_content::{
    find_bios_version, find_ec_in_bios_cap, find_pd_in_bios_cap, find_retimer_version,
};
use crate::ccgx::device::{PdController, PdPort};
#[cfg(not(feature = "uefi"))]
use crate::ccgx::hid::{check_ccg_fw_version, find_devices, DP_CARD_PID, HDMI_CARD_PID};
use crate::ccgx::{self, SiliconId::*};
use crate::chromium_ec;
use crate::chromium_ec::commands::DeckStateMode;
use crate::chromium_ec::commands::FpLedBrightnessLevel;
use crate::chromium_ec::print_err;
use crate::chromium_ec::EcError;
use crate::chromium_ec::EcResult;
#[cfg(feature = "linux")]
use crate::csme;
use crate::ec_binary;
use crate::esrt;
use crate::power;
use crate::smbios;
use crate::smbios::{dmidecode_string_val, get_smbios, is_framework};
#[cfg(feature = "uefi")]
use crate::uefi::enable_page_break;
use crate::util;
#[cfg(not(feature = "uefi"))]
use hidapi::HidApi;
use smbioslib::*;

use crate::chromium_ec::{CrosEc, CrosEcDriverType};

#[cfg(feature = "uefi")]
use core::prelude::rust_2021::derive;

#[cfg_attr(not(feature = "uefi"), derive(clap::ValueEnum))]
#[derive(Clone, Debug, PartialEq)]
pub enum ConsoleArg {
    Recent,
    Follow,
}

#[cfg_attr(not(feature = "uefi"), derive(clap::ValueEnum))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FpBrightnessArg {
    High,
    Medium,
    Low,
}
impl From<FpBrightnessArg> for FpLedBrightnessLevel {
    fn from(w: FpBrightnessArg) -> FpLedBrightnessLevel {
        match w {
            FpBrightnessArg::High => FpLedBrightnessLevel::High,
            FpBrightnessArg::Medium => FpLedBrightnessLevel::Medium,
            FpBrightnessArg::Low => FpLedBrightnessLevel::Low,
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

/// Shadows `clap_std::ClapCli` with extras for UEFI
///
/// The UEFI commandline currently doesn't use clap, so we need to shadow the struct.
/// Also it has extra options.
#[derive(Debug)]
pub struct Cli {
    pub verbosity: log::LevelFilter,
    pub versions: bool,
    pub version: bool,
    pub esrt: bool,
    pub power: bool,
    pub pdports: bool,
    pub privacy: bool,
    pub pd_info: bool,
    pub dp_hdmi_info: bool,
    pub dp_hdmi_update: Option<String>,
    pub audio_card_info: bool,
    pub pd_bin: Option<String>,
    pub ec_bin: Option<String>,
    pub capsule: Option<String>,
    pub dump: Option<String>,
    pub ho2_capsule: Option<String>,
    pub driver: Option<CrosEcDriverType>,
    pub test: bool,
    pub intrusion: bool,
    pub inputmodules: bool,
    pub input_deck_mode: Option<InputDeckModeArg>,
    pub charge_limit: Option<Option<u8>>,
    pub fp_brightness: Option<Option<FpBrightnessArg>>,
    pub kblight: Option<Option<u8>>,
    pub console: Option<ConsoleArg>,
    pub help: bool,
    pub info: bool,
    // UEFI only
    pub allupdate: bool,
    pub paginate: bool,
    // TODO: This is not actually implemented yet
    pub raw_command: Vec<String>,
}

pub fn parse(args: &[String]) -> Cli {
    #[cfg(feature = "uefi")]
    return uefi::parse(args);
    #[cfg(not(feature = "uefi"))]
    return clap_std::parse(args);
}

fn print_single_pd_details(pd: &PdController) {
    if let Ok(si) = pd.get_silicon_id() {
        println!("  Silicon ID:     0x{:X}", si);
    } else {
        println!("  Failed to read Silicon ID/Family");
    }
    if let Ok((mode, frs)) = pd.get_device_info() {
        println!("  Mode:           {:?}", mode);
        println!("  Flash Row Size: {} B", frs);
    } else {
        println!("  Failed to device info");
    }
    pd.print_fw_info();
}

fn print_pd_details(ec: &CrosEc) {
    if !is_framework() {
        println!("Only supported on Framework systems");
        return;
    }
    let pd_01 = PdController::new(PdPort::Left01, ec.clone());
    let pd_23 = PdController::new(PdPort::Right23, ec.clone());

    println!("Left / Ports 01");
    print_single_pd_details(&pd_01);
    println!("Right / Ports 23");
    print_single_pd_details(&pd_23);
}

#[cfg(not(feature = "uefi"))]
const NOT_SET: &str = "NOT SET";

#[cfg(not(feature = "uefi"))]
fn print_audio_card_details() {
    check_synaptics_fw_version();
}

#[cfg(not(feature = "uefi"))]
fn print_dp_hdmi_details() {
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

                println!(
                    "  Serial Number:        {}",
                    dev_info.serial_number().unwrap_or(NOT_SET)
                );
                check_ccg_fw_version(&device);
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
#[cfg(not(feature = "uefi"))]
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

fn print_versions(ec: &CrosEc) {
    println!("UEFI BIOS");
    if let Some(smbios) = get_smbios() {
        let bios_entries = smbios.collect::<SMBiosInformation>();
        let bios = bios_entries.get(0).unwrap();
        println!("  Version:        {}", bios.version());
        println!("  Release Date:   {}", bios.release_date());
    }

    println!("EC Firmware");
    let ver = print_err(ec.version_info()).unwrap_or_else(|| "UNKNOWN".to_string());
    println!("  Build version:  {:?}", ver);

    if let Some((ro, rw, curr)) = ec.flash_version() {
        println!("  RO Version:     {:?}", ro);
        println!("  RW Version:     {:?}", rw);
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

    if let Ok(pd_versions) = ccgx::get_pd_controller_versions(ec) {
        println!("  Right (01)");
        println!(
            "    Main   App:     {}",
            pd_versions.controller01.main_fw.app
        );
        println!(
            "    Backup App:     {}",
            pd_versions.controller01.backup_fw.app
        );
        println!("  Left  (23)");
        println!(
            "    Main   App:     {}",
            pd_versions.controller23.main_fw.app
        );
        println!(
            "    Backup App:     {}",
            pd_versions.controller23.backup_fw.app
        );
    } else if let Ok(pd_versions) = power::read_pd_version(ec) {
        // As fallback try to get it from the EC. But not all EC versions have this command
        println!("  Right (01):     {}", pd_versions.controller01.app);
        println!("  Left  (23):     {}", pd_versions.controller23.app);
    } else {
        println!("  Unknown")
    }

    println!("Retimers");
    let mut found_retimer = false;
    if let Some(esrt) = esrt::get_esrt() {
        for entry in &esrt.entries {
            match entry.fw_class {
                esrt::TGL_RETIMER01_GUID
                | esrt::TGL_RETIMER23_GUID
                | esrt::ADL_RETIMER01_GUID
                | esrt::ADL_RETIMER23_GUID
                | esrt::RPL_RETIMER01_GUID
                | esrt::RPL_RETIMER23_GUID => {
                    if !found_retimer {
                        found_retimer = true;
                    }
                }
                _ => {}
            }
            match entry.fw_class {
                esrt::TGL_RETIMER01_GUID | esrt::ADL_RETIMER01_GUID | esrt::RPL_RETIMER01_GUID => {
                    println!(
                        "  Left:           0x{:X} ({})",
                        entry.fw_version, entry.fw_version
                    );
                }
                esrt::TGL_RETIMER23_GUID | esrt::ADL_RETIMER23_GUID | esrt::RPL_RETIMER23_GUID => {
                    println!(
                        "  Right:          0x{:X} ({})",
                        entry.fw_version, entry.fw_version
                    );
                }
                _ => {}
            }
        }
    }
    if !found_retimer {
        println!("  Unknown");
    }

    #[cfg(feature = "linux")]
    {
        println!("CSME");
        if let Ok(csme) = csme::csme_from_sysfs() {
            println!("  Enabled:        {}", csme.enabled);
            println!("  Version:        {}", csme.version);
        } else {
            println!("  Unknown");
        }
    }
}

fn print_esrt() {
    if let Some(esrt) = esrt::get_esrt() {
        esrt::print_esrt(&esrt);
    } else {
        println!("Could not find and parse ESRT table.");
    }
}

pub fn run_with_args(args: &Cli, _allupdate: bool) -> i32 {
    #[cfg(feature = "uefi")]
    {
        log::set_max_level(args.verbosity);
    }
    #[cfg(not(feature = "uefi"))]
    {
        // TOOD: Should probably have a custom env variable?
        // let env = Env::default()
        //     .filter("FRAMEWORK_COMPUTER_LOG")
        //     .write_style("FRAMEWORK_COMPUTER_LOG_STYLE");

        let level = args.verbosity.as_str();
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level))
            .format_target(false)
            .format_timestamp(None)
            .init();
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
    } else if args.esrt {
        print_esrt();
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
    } else if args.inputmodules {
        println!("Input Module Status:");
        if let Some(status) = print_err(ec.get_input_deck_status()) {
            println!("Input Deck State: {:?}", status.state);
            println!("Touchpad present: {:?}", status.touchpad_present);
            println!("Positions:");
            println!("  Pos 0: {:?}", status.top_row.pos0);
            println!("  Pos 1: {:?}", status.top_row.pos1);
            println!("  Pos 2: {:?}", status.top_row.pos2);
            println!("  Pos 3: {:?}", status.top_row.pos3);
            println!("  Pos 4: {:?}", status.top_row.pos4);
        } else {
            println!("  Unable to tell");
        }
    } else if let Some(mode) = &args.input_deck_mode {
        println!("Set mode to: {:?}", mode);
        ec.set_input_deck_mode((*mode).into()).unwrap();
    } else if let Some(maybe_limit) = args.charge_limit {
        print_err(handle_charge_limit(&ec, maybe_limit));
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
    } else if args.test {
        println!("Self-Test");
        let result = selftest(&ec);
        if result.is_none() {
            return 1;
        }
    } else if args.power {
        power::get_and_print_power_info(&ec);
    } else if args.pdports {
        power::get_and_print_pd_info(&ec);
    } else if args.info {
        smbios_info();
    } else if args.pd_info {
        print_pd_details(&ec);
    } else if args.dp_hdmi_info {
        #[cfg(not(feature = "uefi"))]
        print_dp_hdmi_details();
    } else if let Some(pd_bin_path) = &args.dp_hdmi_update {
        #[cfg(not(feature = "uefi"))]
        flash_dp_hdmi_card(pd_bin_path);
        #[cfg(feature = "uefi")]
        let _ = pd_bin_path;
    } else if args.audio_card_info {
        #[cfg(not(feature = "uefi"))]
        print_audio_card_details();
    } else if args.privacy {
        if let Some((mic, cam)) = print_err(ec.get_privacy_info()) {
            println!(
                "Microphone privacy switch: {}",
                if mic { "Open" } else { "Closed" }
            );
            println!(
                "Camera privacy switch:     {}",
                if cam { "Open" } else { "Closed" }
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
                if header.capsule_guid == esrt::WINUX_GUID {
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
    } else if let Some(capsule_path) = &args.ho2_capsule {
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
                analyze_ec_fw(ec_bin);
            }
            if let Some(pd_bin) = find_pd_in_bios_cap(&data) {
                analyze_ccgx_pd_fw(pd_bin);
            }
        }
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
  -b                         Print output one screen at a time
  -v, --verbose...           More output per occurrence
  -q, --quiet...             Less output per occurrence
      --versions             List current firmware versions
      --version              Show tool version information (Add -vv for more detailed information)
      --esrt                 Display the UEFI ESRT table
      --power                Show current power status (battery and AC)
      --pdports              Show information about USB-C PD ports
      --info                 Show info from SMBIOS (Only on UEFI)
      --pd-info              Show details about the PD controllers
      --privacy              Show privacy switch statuses (camera and microphone)
      --pd-bin <PD_BIN>      Parse versions from PD firmware binary file
      --ec-bin <EC_BIN>      Parse versions from EC firmware binary file
      --capsule <CAPSULE>    Parse UEFI Capsule information from binary file
      --intrusion            Show status of intrusion switch
      --inputmodules         Show status of the input modules (Framework 16 only)
      --charge-limit [<VAL>] Get or set battery charge limit (Percentage number as arg, e.g. '100')
      --fp-brightness [<VAL>]Get or set fingerprint LED brightness level [possible values: high, medium, low]
      --kblight [<KBLIGHT>]  Set keyboard backlight percentage or get, if no value provided
      --console <CONSOLE>    Get EC console, choose whether recent or to follow the output [possible values: recent, follow]
  -t, --test                 Run self-test to check if interaction with EC is possible
  -h, --help                 Print help information
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

fn selftest(ec: &CrosEc) -> Option<()> {
    println!(
        "  SMBIOS Platform:     {:?}",
        smbios::get_platform().unwrap()
    );
    println!("  SMBIOS is_framework: {}", smbios::is_framework());

    println!("  Dump EC memory region");
    if let Some(mem) = ec.dump_mem_region() {
        util::print_multiline_buffer(&mem, 0);
    } else {
        println!("    Failed to read EC memory region")
    }

    println!("  Checking EC memory mapped magic bytes");
    ec.check_mem_magic()?;

    println!("  Reading EC Build Version");
    print_err(ec.version_info())?;

    println!("  Reading EC Flash");
    ec.flash_version()?;

    println!("  Getting power info from EC");
    power::power_info(ec)?;

    println!("  Getting AC info from EC");
    // All our laptops have at least 4 PD ports so far
    if power::get_pd_info(ec, 4).iter().any(|x| x.is_err()) {
        return None;
    }

    // Try to get PD versions through EC
    power::read_pd_version(ec).ok()?;

    let pd_01 = PdController::new(PdPort::Left01, ec.clone());
    let pd_23 = PdController::new(PdPort::Right23, ec.clone());
    println!("  Getting PD01 info");
    print_err(pd_01.get_silicon_id())?;
    print_err(pd_01.get_device_info())?;
    print_err(pd_01.get_fw_versions())?;
    println!("  Getting PD23 info");
    print_err(pd_23.get_silicon_id())?;
    print_err(pd_23.get_device_info())?;
    print_err(pd_23.get_fw_versions())?;

    Some(())
}

fn smbios_info() {
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
                    println!("  Version:      {}", version);
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
                if let Some(family) = dmidecode_string_val(&data.family()) {
                    println!("  Family:       {}", family);
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
        return;
    } else if let Some(versions) = ccgx::binary::read_versions(data, Ccg6) {
        println!("Detected CCG6 firmware");
        println!("FW 1 (Backup)");
        ccgx::binary::print_fw(&versions.backup_fw);

        println!("FW 2 (Main)");
        ccgx::binary::print_fw(&versions.main_fw);
        return;
    } else {
        println!("Failed to read versions")
    }
}

pub fn analyze_ec_fw(data: &[u8]) {
    // Readonly firmware
    if let Some(ver) = ec_binary::read_ec_version(data, true) {
        ec_binary::print_ec_version(&ver, true);
    } else {
        println!("Failed to read version")
    }
    // Readwrite firmware
    if let Some(ver) = ec_binary::read_ec_version(data, false) {
        ec_binary::print_ec_version(&ver, false);
    } else {
        println!("Failed to read version")
    }
}

pub fn analyze_capsule(data: &[u8]) -> Option<capsule::EfiCapsuleHeader> {
    let header = capsule::parse_capsule_header(data)?;
    capsule::print_capsule_header(&header);

    match header.capsule_guid {
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
            println!("  Type:    Framework TGL Retimer01 (Left)");
        }
        esrt::TGL_RETIMER23_GUID => {
            println!("  Type:   Framework TGL Retimer23 (Right)");
        }
        esrt::ADL_RETIMER01_GUID => {
            println!("  Type:    Framework ADL Retimer01 (Left)");
        }
        esrt::ADL_RETIMER23_GUID => {
            println!("  Type:   Framework ADL Retimer23 (Right)");
        }
        esrt::RPL_RETIMER01_GUID => {
            println!("  Type:    Framework RPL Retimer01 (Left)");
        }
        esrt::RPL_RETIMER23_GUID => {
            println!("  Type:   Framework RPL Retimer23 (Right)");
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
        // Prevent accidentally setting a very low limit
        if limit < 25 {
            return Err(EcError::DeviceError(
                "Not recommended to set charge limit below 25%".to_string(),
            ));
        }
        ec.set_charge_limit(cur_min, limit)?;
    }

    let (min, max) = ec.get_charge_limit()?;
    println!("Minimum {}%, Maximum {}%", min, max);

    Ok(())
}

fn handle_fp_brightness(ec: &CrosEc, maybe_brightness: Option<FpBrightnessArg>) -> EcResult<()> {
    if let Some(brightness) = maybe_brightness {
        ec.set_fp_led_level(brightness.into())?;
    }

    let level = ec.get_fp_led_level()?;
    println!("Fingerprint LED Brightness: {:?}%", level);

    Ok(())
}
