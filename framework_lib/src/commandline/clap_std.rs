//! Module to factor out commandline interaction
//! This way we can use it in the regular OS commandline tool on Linux and Windows,
//! as well as on the UEFI shell tool.
use clap::error::ErrorKind;
use clap::Parser;
use clap::{arg, command, Arg, Args, FromArgMatches};
use clap_num::maybe_hex;

use crate::chromium_ec::commands::SetGpuSerialMagic;
use crate::chromium_ec::CrosEcDriverType;
use crate::commandline::{
    Cli, ConsoleArg, FpBrightnessArg, HardwareDeviceType, InputDeckModeArg, LogLevel, RebootEcArg,
    TabletModeArg,
};

/// Swiss army knife for Framework laptops
#[derive(Parser)]
#[command(arg_required_else_help = true)]
struct ClapCli {
    #[command(flatten)]
    verbosity: clap_verbosity_flag::Verbosity,

    /// List current firmware versions
    #[arg(long)]
    versions: bool,

    /// Show tool version information (Add -vv for more details)
    #[arg(long)]
    version: bool,

    /// Show features support by the firmware
    #[arg(long)]
    features: bool,

    /// Display the UEFI ESRT table
    #[arg(long)]
    esrt: bool,

    // Device type to compare_version string with version string on device
    #[clap(value_enum)]
    #[arg(long)]
    device: Option<HardwareDeviceType>,

    // version to compare with
    #[arg(long)]
    compare_version: Option<String>,

    /// Show current power status of battery and AC (Add -vv for more details)
    #[arg(long)]
    power: bool,

    /// Print thermal information (Temperatures and Fan speed)
    #[arg(long)]
    thermal: bool,

    /// Print sensor information (ALS, G-Sensor)
    #[arg(long)]
    sensors: bool,

    /// Set fan duty cycle (0-100%)
    #[clap(num_args=..=2)]
    #[arg(long)]
    fansetduty: Vec<u32>,

    /// Set fan RPM (limited by EC fan table max RPM)
    #[clap(num_args=..=2)]
    #[arg(long)]
    fansetrpm: Vec<u32>,

    /// Turn on automatic fan speed control
    #[arg(long)]
    autofanctrl: bool,

    /// Show information about USB-C PD ports
    #[arg(long)]
    pdports: bool,

    /// Show info from SMBIOS (Only on UEFI)
    #[arg(long)]
    info: bool,

    /// Show details about the PD controllers
    #[arg(long)]
    pd_info: bool,

    /// Reset a specific PD controller (for debugging only)
    #[arg(long)]
    pd_reset: Option<u8>,

    /// Disable all ports on a specific PD controller (for debugging only)
    #[arg(long)]
    pd_disable: Option<u8>,

    /// Enable all ports on a specific PD controller (for debugging only)
    #[arg(long)]
    pd_enable: Option<u8>,

    /// Show details about connected DP or HDMI Expansion Cards
    #[arg(long)]
    dp_hdmi_info: bool,

    /// Update the DisplayPort or HDMI Expansion Card
    #[arg(long, value_name = "UPDATE_BIN")]
    dp_hdmi_update: Option<std::path::PathBuf>,

    /// Show details about connected Audio Expansion Cards (Needs root privileges)
    #[arg(long)]
    audio_card_info: bool,

    /// Show privacy switch statuses (camera and microphone)
    #[arg(long)]
    privacy: bool,

    /// Parse versions from PD firmware binary file
    #[arg(long)]
    pd_bin: Option<std::path::PathBuf>,

    /// Parse versions from EC firmware binary file
    #[arg(long)]
    ec_bin: Option<std::path::PathBuf>,

    /// Parse UEFI Capsule information from binary file
    #[arg(long)]
    capsule: Option<std::path::PathBuf>,

    /// Dump extracted UX capsule bitmap image to a file
    #[arg(long)]
    dump: Option<std::path::PathBuf>,

    /// Parse UEFI Capsule information from binary file
    #[arg(long)]
    h2o_capsule: Option<std::path::PathBuf>,

    /// Dump EC flash contents
    #[arg(long)]
    dump_ec_flash: Option<std::path::PathBuf>,

    /// Flash EC (RO+RW) with new firmware from file - may render your hardware unbootable!
    #[arg(long)]
    flash_ec: Option<std::path::PathBuf>,

    /// Flash EC with new RO firmware from file - may render your hardware unbootable!
    #[arg(long)]
    flash_ro_ec: Option<std::path::PathBuf>,

    /// Flash EC with new RW firmware from file
    #[arg(long)]
    flash_rw_ec: Option<std::path::PathBuf>,

    /// Show status of intrusion switch
    #[arg(long)]
    intrusion: bool,

    /// Show status of the input modules (Framework 16 only)
    #[arg(long)]
    inputdeck: bool,

    /// Set input deck power mode [possible values: auto, off, on] (Framework 16 only)
    #[arg(long)]
    inputdeck_mode: Option<InputDeckModeArg>,

    /// Show status of the expansion bay (Framework 16 only)
    #[arg(long)]
    expansion_bay: bool,

    /// Get or set max charge limit
    #[arg(long)]
    charge_limit: Option<Option<u8>>,

    /// Set max charge current limit
    #[arg(long)]
    #[clap(num_args = 1..=2)]
    charge_current_limit: Vec<u32>,

    /// Set max charge current limit
    #[arg(long)]
    #[clap(num_args = 1..=2)]
    charge_rate_limit: Vec<f32>,

    /// Get GPIO value by name or all, if no name provided
    #[arg(long)]
    get_gpio: Option<Option<String>>,

    /// Get or set fingerprint LED brightness level
    #[arg(long)]
    fp_led_level: Option<Option<FpBrightnessArg>>,

    /// Get or set fingerprint LED brightness percentage
    #[arg(long)]
    fp_brightness: Option<Option<u8>>,

    /// Set keyboard backlight percentage or get, if no value provided
    #[arg(long)]
    kblight: Option<Option<u8>>,

    /// Remap a key by changing the scancode
    #[arg(long, value_parser=maybe_hex::<u16>)]
    #[clap(num_args = 3)]
    remap_key: Vec<u16>,

    /// Set the color of <key> to <RGB>. Multiple colors for adjacent keys can be set at once.
    /// <key> <RGB> [<RGB> ...]
    /// Example: 0 0xFF000 0x00FF00 0x0000FF
    #[clap(num_args = 2..)]
    #[arg(long, value_parser=maybe_hex::<u64>, value_names(["START", "HEXCOLOR"]))]
    rgbkbd: Vec<u64>,

    /// Control PS2 touchpad emulation (DEBUG COMMAND, if touchpad not working, reboot system)
    #[clap(value_enum, hide(true))]
    #[arg(long)]
    ps2_enable: Option<bool>,

    /// Set tablet mode override
    #[clap(value_enum)]
    #[arg(long)]
    tablet_mode: Option<TabletModeArg>,

    /// Enable/disable touchscreen
    #[clap(value_enum)]
    #[arg(long)]
    touchscreen_enable: Option<bool>,

    /// Check stylus battery level (USI 2.0 stylus only)
    #[clap(value_enum)]
    #[arg(long)]
    stylus_battery: bool,

    /// Get EC console, choose whether recent or to follow the output
    #[clap(value_enum)]
    #[arg(long)]
    console: Option<ConsoleArg>,

    /// Control EC RO/RW jump
    #[clap(value_enum)]
    #[arg(long)]
    reboot_ec: Option<RebootEcArg>,

    /// Get or set EC hibernate delay (S5 to G3)
    #[clap(value_enum)]
    #[arg(long)]
    ec_hib_delay: Option<Option<u32>>,

    /// Hash a file of arbitrary data
    #[arg(long)]
    hash: Option<std::path::PathBuf>,

    /// Select which driver is used. By default portio is used
    #[clap(value_enum)]
    #[arg(long)]
    driver: Option<CrosEcDriverType>,

    /// Specify I2C addresses of the PD chips (Advanced)
    #[clap(number_of_values = 3, requires("pd_ports"))]
    #[arg(long)]
    pd_addrs: Vec<u16>,

    /// Specify I2C ports of the PD chips (Advanced)
    #[clap(number_of_values = 3, requires("pd_addrs"))]
    #[arg(long)]
    pd_ports: Vec<u8>,

    /// Run self-test to check if interaction with EC is possible
    #[arg(long, short)]
    test: bool,

    /// Force execution of an unsafe command - may render your hardware unbootable!
    #[arg(long, short)]
    force: bool,

    /// Simulate execution of a command (e.g. --flash-ec)
    #[arg(long)]
    dry_run: bool,

    /// File to write to the gpu EEPROM
    #[arg(long)]
    flash_gpu_descriptor_file: Option<std::path::PathBuf>,

    /// File to dump the gpu EEPROM to
    #[arg(long)]
    dump_gpu_descriptor_file: Option<std::path::PathBuf>,
}

/// Parse a list of commandline arguments and return the struct
pub fn parse(args: &[String]) -> Cli {
    // Step 1 - Define args that can't be derived
    let cli = command!()
        .arg(Arg::new("fgd").long("flash-gpu-descriptor").num_args(2))
        .disable_version_flag(true);
    // Step 2 - Define args from derived struct
    let mut cli = ClapCli::augment_args(cli);

    // Step 3 - Parse args that can't be derived
    let matches = cli.clone().get_matches_from(args);
    let fgd = matches
        .get_many::<String>("fgd")
        .unwrap_or_default()
        .map(|v| v.as_str())
        .collect::<Vec<_>>();
    let flash_gpu_descriptor = if !fgd.is_empty() {
        let hex_magic = if let Some(hex_magic) = fgd[0].strip_prefix("0x") {
            u8::from_str_radix(hex_magic, 16)
        } else {
            // Force parse error
            u8::from_str_radix("", 16)
        };

        let magic = if let Ok(magic) = fgd[0].parse::<u8>() {
            magic
        } else if let Ok(hex_magic) = hex_magic {
            hex_magic
        } else if fgd[0].to_uppercase() == "GPU" {
            SetGpuSerialMagic::WriteGPUConfig as u8
        } else if fgd[0].to_uppercase() == "SSD" {
            SetGpuSerialMagic::WriteSSDConfig as u8
        } else {
            cli.error(
                ErrorKind::InvalidValue,
                "First argument of --flash-gpu-descriptor must be an integer or one of: 'GPU', 'SSD'",
            )
            .exit();
        };
        if fgd[1].len() != 18 {
            cli.error(
                ErrorKind::InvalidValue,
                "Second argument of --flash-gpu-descriptor must be an 18 digit serial number",
            )
            .exit();
        }
        Some((magic, fgd[1].to_string()))
    } else {
        None
    };

    // Step 4 - Parse from derived struct
    let args = ClapCli::from_arg_matches(&matches)
        .map_err(|err| err.exit())
        .unwrap();

    let pd_addrs = match args.pd_addrs.len() {
        3 => Some((args.pd_addrs[0], args.pd_addrs[1], args.pd_addrs[2])),
        0 => None,
        // Checked by clap
        _ => unreachable!(),
    };
    let pd_ports = match args.pd_ports.len() {
        3 => Some((args.pd_ports[0], args.pd_ports[1], args.pd_ports[2])),
        0 => None,
        // Checked by clap
        _ => unreachable!(),
    };
    let fansetduty = match args.fansetduty.len() {
        2 => Some((Some(args.fansetduty[0]), args.fansetduty[1])),
        1 => Some((None, args.fansetduty[0])),
        _ => None,
    };
    let fansetrpm = match args.fansetrpm.len() {
        2 => Some((Some(args.fansetrpm[0]), args.fansetrpm[1])),
        1 => Some((None, args.fansetrpm[0])),
        _ => None,
    };
    let charge_current_limit = match args.charge_current_limit.len() {
        2 => Some((
            args.charge_current_limit[0],
            Some(args.charge_current_limit[1]),
        )),
        1 => Some((args.charge_current_limit[0], None)),
        _ => None,
    };
    let charge_rate_limit = match args.charge_rate_limit.len() {
        2 => Some((args.charge_rate_limit[0], Some(args.charge_rate_limit[1]))),
        1 => Some((args.charge_rate_limit[0], None)),
        _ => None,
    };
    let remap_key = match args.remap_key.len() {
        3 => Some((
            args.remap_key[0] as u8,
            args.remap_key[1] as u8,
            args.remap_key[2],
        )),
        _ => None,
    };

    Cli {
        verbosity: LogLevel(args.verbosity.log_level_filter()),
        versions: args.versions,
        version: args.version,
        features: args.features,
        esrt: args.esrt,
        device: args.device,
        compare_version: args.compare_version,
        power: args.power,
        thermal: args.thermal,
        sensors: args.sensors,
        fansetduty,
        fansetrpm,
        autofanctrl: args.autofanctrl,
        pdports: args.pdports,
        pd_info: args.pd_info,
        pd_reset: args.pd_reset,
        pd_disable: args.pd_disable,
        pd_enable: args.pd_enable,
        dp_hdmi_info: args.dp_hdmi_info,
        dp_hdmi_update: args
            .dp_hdmi_update
            .map(|x| x.into_os_string().into_string().unwrap()),
        audio_card_info: args.audio_card_info,
        privacy: args.privacy,
        pd_bin: args
            .pd_bin
            .map(|x| x.into_os_string().into_string().unwrap()),
        ec_bin: args
            .ec_bin
            .map(|x| x.into_os_string().into_string().unwrap()),
        capsule: args
            .capsule
            .map(|x| x.into_os_string().into_string().unwrap()),
        dump: args.dump.map(|x| x.into_os_string().into_string().unwrap()),
        h2o_capsule: args
            .h2o_capsule
            .map(|x| x.into_os_string().into_string().unwrap()),
        dump_ec_flash: args
            .dump_ec_flash
            .map(|x| x.into_os_string().into_string().unwrap()),
        flash_ec: args
            .flash_ec
            .map(|x| x.into_os_string().into_string().unwrap()),
        flash_ro_ec: args
            .flash_ro_ec
            .map(|x| x.into_os_string().into_string().unwrap()),
        flash_rw_ec: args
            .flash_rw_ec
            .map(|x| x.into_os_string().into_string().unwrap()),
        intrusion: args.intrusion,
        inputdeck: args.inputdeck,
        inputdeck_mode: args.inputdeck_mode,
        expansion_bay: args.expansion_bay,
        charge_limit: args.charge_limit,
        charge_current_limit,
        charge_rate_limit,
        get_gpio: args.get_gpio,
        fp_led_level: args.fp_led_level,
        fp_brightness: args.fp_brightness,
        kblight: args.kblight,
        remap_key,
        rgbkbd: args.rgbkbd,
        ps2_enable: args.ps2_enable,
        tablet_mode: args.tablet_mode,
        touchscreen_enable: args.touchscreen_enable,
        stylus_battery: args.stylus_battery,
        console: args.console,
        reboot_ec: args.reboot_ec,
        ec_hib_delay: args.ec_hib_delay,
        hash: args.hash.map(|x| x.into_os_string().into_string().unwrap()),
        driver: args.driver,
        pd_addrs,
        pd_ports,
        test: args.test,
        dry_run: args.dry_run,
        force: args.force,
        // TODO: Set help. Not very important because Clap handles this by itself
        help: false,
        // UEFI only for now. Don't need to handle
        allupdate: false,
        // UEFI only - every command needs to implement a parameter to enable the pager
        paginate: false,
        info: args.info,
        flash_gpu_descriptor,
        flash_gpu_descriptor_file: args
            .flash_gpu_descriptor_file
            .map(|x| x.into_os_string().into_string().unwrap()),
        dump_gpu_descriptor_file: args
            .dump_gpu_descriptor_file
            .map(|x| x.into_os_string().into_string().unwrap()),
        raw_command: vec![],
    }
}
