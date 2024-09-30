//! Module to factor out commandline interaction
//! This way we can use it in the regular OS commandline tool on Linux and Windows,
//! as well as on the UEFI shell tool.
use clap::Parser;

use crate::chromium_ec::CrosEcDriverType;
use crate::commandline::{Cli, ConsoleArg, FpBrightnessArg, InputDeckModeArg, RebootEcArg};

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

    /// Display the UEFI ESRT table
    #[arg(long)]
    esrt: bool,

    /// Show current power status of battery and AC (Add -vv for more details)
    #[arg(long)]
    power: bool,

    /// Print thermal information (Temperatures and Fan speed)
    #[arg(long)]
    thermal: bool,

    /// Print sensor information (ALS, G-Sensor)
    #[arg(long)]
    sensors: bool,

    /// Show information about USB-C PD ports
    #[arg(long)]
    pdports: bool,

    /// Show info from SMBIOS
    #[arg(long)]
    info: bool,

    /// Show info about system serial numbers
    #[arg(long)]
    serialnums: bool,

    /// Show details about the PD controllers
    #[arg(long)]
    pd_info: bool,

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
    ho2_capsule: Option<std::path::PathBuf>,

    /// Dump EC flash contents
    #[arg(long)]
    dump_ec_flash: Option<std::path::PathBuf>,

    /// Flash EC with new firmware from file
    #[arg(long)]
    flash_ec: Option<std::path::PathBuf>,

    /// Flash EC with new RO firmware from file
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
    inputmodules: bool,

    /// Set input deck power mode [possible values: auto, off, on] (Framework 16 only)
    #[arg(long)]
    input_deck_mode: Option<InputDeckModeArg>,

    /// Get or set max charge limit
    #[arg(long)]
    charge_limit: Option<Option<u8>>,

    /// Get or set fingerprint LED brightness
    #[arg(long)]
    fp_brightness: Option<Option<FpBrightnessArg>>,

    /// Set keyboard backlight percentage or get, if no value provided
    #[arg(long)]
    kblight: Option<Option<u8>>,

    /// Get EC console, choose whether recent or to follow the output
    #[clap(value_enum)]
    #[arg(long)]
    console: Option<ConsoleArg>,

    /// Control EC RO/RW jump
    #[clap(value_enum)]
    #[arg(long)]
    reboot_ec: Option<RebootEcArg>,

    /// Hash a file of arbitrary data
    #[arg(long)]
    hash: Option<std::path::PathBuf>,

    /// Select which driver is used. By default portio is used
    #[clap(value_enum)]
    #[arg(long)]
    driver: Option<CrosEcDriverType>,

    /// Specify I2C addresses of the PD chips (Advanced)
    #[clap(number_of_values = 2, requires("pd_ports"), requires("has_mec"))]
    #[arg(long)]
    pd_addrs: Vec<u16>,

    /// Specify I2C ports of the PD chips (Advanced)
    #[clap(number_of_values = 2, requires("pd_addrs"), requires("has_mec"))]
    #[arg(long)]
    pd_ports: Vec<u8>,

    /// Specify the type of EC chip (MEC/MCHP or other)
    #[clap(requires("pd_addrs"), requires("pd_ports"))]
    #[arg(long)]
    has_mec: Option<bool>,

    /// Run self-test to check if interaction with EC is possible
    #[arg(long, short)]
    test: bool,
}

/// Parse a list of commandline arguments and return the struct
pub fn parse(args: &[String]) -> Cli {
    let args = ClapCli::parse_from(args);

    let pd_addrs = match args.pd_addrs.len() {
        2 => Some((args.pd_addrs[0], args.pd_addrs[1])),
        0 => None,
        _ => {
            // Actually unreachable, checked by clap
            println!(
                "Must provide exactly to PD Addresses. Provided: {:?}",
                args.pd_addrs
            );
            std::process::exit(1);
        }
    };
    let pd_ports = match args.pd_ports.len() {
        2 => Some((args.pd_ports[0], args.pd_ports[1])),
        0 => None,
        _ => {
            // Actually unreachable, checked by clap
            println!(
                "Must provide exactly to PD Ports. Provided: {:?}",
                args.pd_ports
            );
            std::process::exit(1);
        }
    };

    Cli {
        verbosity: args.verbosity.log_level_filter(),
        versions: args.versions,
        version: args.version,
        esrt: args.esrt,
        power: args.power,
        thermal: args.thermal,
        sensors: args.sensors,
        pdports: args.pdports,
        pd_info: args.pd_info,
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
        ho2_capsule: args
            .ho2_capsule
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
        inputmodules: args.inputmodules,
        input_deck_mode: args.input_deck_mode,
        charge_limit: args.charge_limit,
        fp_brightness: args.fp_brightness,
        kblight: args.kblight,
        console: args.console,
        reboot_ec: args.reboot_ec,
        hash: args.hash.map(|x| x.into_os_string().into_string().unwrap()),
        driver: args.driver,
        pd_addrs,
        pd_ports,
        has_mec: args.has_mec,
        test: args.test,
        // TODO: Set help. Not very important because Clap handles this by itself
        help: false,
        // UEFI only for now. Don't need to handle
        allupdate: false,
        // UEFI only - every command needs to implement a parameter to enable the pager
        paginate: false,
        info: args.info,
        serialnums: args.serialnums,
        raw_command: vec![],
    }
}
