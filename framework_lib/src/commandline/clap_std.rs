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

    /// Show information about USB-C PD ports
    #[arg(long)]
    pdports: bool,

    /// Show info from SMBIOS (Only on UEFI)
    #[arg(long)]
    info: bool,

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

    /// Show status of intrusion switch
    #[arg(long)]
    intrusion: bool,

    /// Show status of the input modules (Framework 16 only)
    #[arg(long)]
    inputmodules: bool,

    /// Show status of the input modules (Framework 16 only)
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

    /// Select which driver is used. By default portio is used
    #[clap(value_enum)]
    #[arg(long)]
    driver: Option<CrosEcDriverType>,

    /// Run self-test to check if interaction with EC is possible
    #[arg(long, short)]
    test: bool,
}

/// Parse a list of commandline arguments and return the struct
pub fn parse(args: &[String]) -> Cli {
    let args = ClapCli::parse_from(args);

    Cli {
        verbosity: args.verbosity.log_level_filter(),
        versions: args.versions,
        version: args.version,
        esrt: args.esrt,
        power: args.power,
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
        intrusion: args.intrusion,
        inputmodules: args.inputmodules,
        input_deck_mode: args.input_deck_mode,
        charge_limit: args.charge_limit,
        fp_brightness: args.fp_brightness,
        kblight: args.kblight,
        console: args.console,
        reboot_ec: args.reboot_ec,
        driver: args.driver,
        test: args.test,
        // TODO: Set help. Not very important because Clap handles this by itself
        help: false,
        // UEFI only for now. Don't need to handle
        allupdate: false,
        // UEFI only - every command needs to implement a parameter to enable the pager
        paginate: false,
        info: args.info,
        raw_command: vec![],
    }
}
