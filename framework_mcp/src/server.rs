//! The MCP tools, one per piece of hardware information
//!
//! Every tool reads from the EC through framework_lib and returns the
//! result as JSON, both as text and as structured content. Hardware
//! failures are reported as tool errors (`is_error`), so the assistant
//! sees what went wrong, protocol errors are reserved for bugs.

use dmidecode::Structure;
use framework_lib::ccgx::{self, ControllerFirmwares, PdVersions};
use framework_lib::chromium_ec::{CrosEc, EcError};
use framework_lib::{power, smbios, Platform, PlatformFamily};
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler};
use serde::Serialize;
use serde_json::{json, Value};

type ToolResult = Result<CallToolResult, ErrorData>;

/// Format an EC error for the assistant
fn ec_err(err: EcError) -> String {
    format!("EC error: {:?}", err)
}

/// Serialize a value, or explain why it could not be read
fn section<T: Serialize>(result: Result<T, String>) -> Value {
    match result {
        Ok(value) => serde_json::to_value(value)
            .unwrap_or_else(|err| json!({ "error": format!("Failed to serialize: {}", err) })),
        Err(err) => json!({ "error": err }),
    }
}

/// Turn a read result into a tool result
///
/// Successful reads carry the JSON both as pretty text and as structured
/// content, failed reads become tool errors. Structured content must be a
/// JSON object, so anything else is wrapped in one.
fn to_tool_result<T: Serialize>(result: Result<T, String>) -> ToolResult {
    match result {
        Ok(value) => {
            let value = serde_json::to_value(value).map_err(|err| {
                ErrorData::internal_error(format!("Failed to serialize: {}", err), None)
            })?;
            let value = if value.is_object() {
                value
            } else {
                json!({ "value": value })
            };
            let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
            let mut result = CallToolResult::structured(value);
            result.content = vec![ContentBlock::text(text)];
            Ok(result)
        }
        Err(err) => Ok(CallToolResult::error(vec![ContentBlock::text(err)])),
    }
}

/// Which system this is
#[derive(Debug, Serialize)]
struct PlatformInfo {
    /// Whether this looks like a Framework Computer system at all
    is_framework: bool,
    /// Mainboard, e.g. Framework 13 with AMD Ryzen AI 300
    platform: Option<Platform>,
    /// Product family, e.g. Framework 13
    family: Option<PlatformFamily>,
    /// Product name from SMBIOS
    product_name: Option<String>,
    /// Mainboard revision from SMBIOS
    baseboard_version: Option<String>,
}

fn platform_info() -> PlatformInfo {
    PlatformInfo {
        is_framework: smbios::is_framework(),
        platform: smbios::get_platform(),
        family: smbios::get_family(),
        product_name: smbios::get_product_name(),
        baseboard_version: smbios::get_baseboard_version().map(|v| format!("{:?}", v)),
    }
}

#[derive(Debug, Serialize)]
struct BiosVersion {
    version: String,
    release_date: String,
}

#[derive(Debug, Serialize)]
struct EcVersions {
    /// Full build string of the running firmware
    build: String,
    /// Version of the read-only image
    ro: Option<String>,
    /// Version of the read-write image
    rw: Option<String>,
    /// Which image is running, RO or RW
    current_image: Option<String>,
}

#[derive(Debug, Serialize)]
struct FwVersion {
    /// Version of the silicon vendor's base firmware
    base: String,
    /// Version of Framework's application firmware, this is what --versions shows
    app: String,
}

impl From<ccgx::ControllerVersion> for FwVersion {
    fn from(version: ccgx::ControllerVersion) -> Self {
        FwVersion {
            base: version.base.to_string(),
            app: version.app.to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
struct PdControllerVersions {
    /// Which controller, e.g. "Right (01)"
    controller: String,
    /// Which image is running: MainFw, BackupFw or BootLoader
    active_image: String,
    /// Version of the running image, formatted like --versions does
    active_version: String,
    main: FwVersion,
    backup: FwVersion,
    bootloader: FwVersion,
}

impl PdControllerVersions {
    fn new(controller: &str, fw: &ControllerFirmwares) -> Self {
        PdControllerVersions {
            controller: controller.to_string(),
            active_image: format!("{:?}", fw.active_fw),
            active_version: fw.active_fw_ver(),
            main: fw.main_fw.into(),
            backup: fw.backup_fw.into(),
            bootloader: fw.bootloader.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct Versions {
    bios: Option<BiosVersion>,
    /// EcVersions, or an object with an "error" field
    ec: Value,
    /// List of PdControllerVersions, or an object with an "error" field
    pd_controllers: Value,
}

fn versions(ec: &CrosEc) -> Versions {
    let bios = smbios::get_smbios().and_then(|smbios| {
        smbios.structures().find_map(|s| match s {
            Ok(Structure::Bios(bios)) => Some(BiosVersion {
                version: bios.bios_version.to_string(),
                release_date: bios.bios_release_date.to_string(),
            }),
            _ => None,
        })
    });

    let ec_versions = ec.version_info().map_err(ec_err).map(|build| {
        let flash = ec.flash_version();
        EcVersions {
            build,
            ro: flash.as_ref().map(|(ro, _, _)| ro.clone()),
            rw: flash.as_ref().map(|(_, rw, _)| rw.clone()),
            current_image: flash
                .as_ref()
                .map(|(_, _, current)| format!("{:?}", current)),
        }
    });

    // Locating the PD controllers needs to know the platform, the library
    // panics otherwise
    let pd_controllers = if smbios::get_platform().is_none() {
        Err("Unknown platform, can't locate the PD controllers".to_string())
    } else {
        ccgx::get_pd_controller_versions(ec).map_err(ec_err)
    }
    .map(|versions| match versions {
        PdVersions::RightLeft((right, left)) => vec![
            PdControllerVersions::new("Right (01)", &right),
            PdControllerVersions::new("Left (23)", &left),
        ],
        PdVersions::Single(pd) => vec![PdControllerVersions::new("PD", &pd)],
        PdVersions::Many(pds) => pds
            .iter()
            .enumerate()
            .map(|(i, pd)| PdControllerVersions::new(&format!("PD {}", i), pd))
            .collect(),
    });

    Versions {
        bios,
        ec: section(ec_versions),
        pd_controllers: section(pd_controllers),
    }
}

/// Wrapper because structured tool results must be JSON objects, not arrays
#[derive(Debug, Serialize)]
struct Ports<T> {
    ports: Vec<T>,
}

#[derive(Debug, Serialize)]
struct Thresholds<T> {
    sensors: Vec<T>,
}

#[derive(Debug, Serialize)]
struct Panic<T> {
    /// null if the EC never panicked
    panic: Option<T>,
}

#[derive(Debug, Serialize)]
struct ChargeLimit {
    /// Charging starts below this percentage
    min_percent: u8,
    /// Charging stops at this percentage
    max_percent: u8,
}

#[derive(Debug, Serialize)]
struct PrivacySwitches {
    /// Whether the microphone is connected (hardware privacy switch)
    microphone_connected: bool,
    /// Whether the camera is connected (hardware privacy switch)
    camera_connected: bool,
}

/// The MCP server, one instance per client connection
#[derive(Clone)]
pub struct FrameworkServer {
    ec: CrosEc,
}

impl FrameworkServer {
    pub fn new(ec: CrosEc) -> Self {
        FrameworkServer { ec }
    }

    /// Run a synchronous hardware read off the async runtime and turn the
    /// result into a tool result
    async fn read<T, F>(&self, read: F) -> ToolResult
    where
        T: Serialize + Send + 'static,
        F: FnOnce(&CrosEc) -> Result<T, String> + Send + 'static,
    {
        let ec = self.ec.clone();
        match tokio::task::spawn_blocking(move || read(&ec)).await {
            Ok(result) => to_tool_result(result),
            Err(err) => Err(ErrorData::internal_error(
                format!("Hardware access failed: {}", err),
                None,
            )),
        }
    }
}

// Every tool has a matching framework_tool command, named in the description
// so a user can reproduce what the assistant saw.
#[tool_router]
impl FrameworkServer {
    #[tool(
        description = "Which Framework Computer system this is: platform, product family, product name and mainboard revision from SMBIOS.",
        annotations(title = "Platform", read_only_hint = true)
    )]
    async fn platform(&self) -> ToolResult {
        self.read(|_ec| Ok(platform_info())).await
    }

    #[tool(
        description = "Firmware versions of BIOS, Embedded Controller (RO/RW image) and the USB-C PD controllers. Same data as `framework_tool --versions`.",
        annotations(title = "Firmware Versions", read_only_hint = true)
    )]
    async fn versions(&self) -> ToolResult {
        self.read(|ec| Ok(versions(ec))).await
    }

    #[tool(
        description = "Battery and charger state: AC present, battery cutoff (ship mode), charge percentage, remaining/full/design capacity in mAh, voltage in mV, charge/discharge rate in mA, cycle count, charger voltage/current and input current limit. Same as `framework_tool --power`.",
        annotations(title = "Power Status", read_only_hint = true)
    )]
    async fn power_status(&self) -> ToolResult {
        self.read(|ec| Ok(power::get_power_status(ec))).await
    }

    #[tool(
        description = "Battery charge limit in percent: charging stops at max and only resumes below min. Same as `framework_tool --charge-limit`.",
        annotations(title = "Charge Limit", read_only_hint = true)
    )]
    async fn charge_limit(&self) -> ToolResult {
        self.read(|ec| {
            ec.get_charge_limit()
                .map(|(min, max)| ChargeLimit {
                    min_percent: min,
                    max_percent: max,
                })
                .map_err(ec_err)
        })
        .await
    }

    #[tool(
        description = "Temperature sensors in degrees Celsius, fan speeds in RPM (or Stalled/NotPresent) and whether the EC throttles the CPU. Same as `framework_tool --thermal`.",
        annotations(title = "Thermal", read_only_hint = true)
    )]
    async fn thermal(&self) -> ToolResult {
        self.read(|ec| power::get_thermal(ec).map_err(ec_err)).await
    }

    #[tool(
        description = "Per-sensor thermal thresholds in degrees Celsius: warn, high (CPU throttle), halt (shutdown), fan_off and fan_max. null means the threshold is disabled. Returns {\"sensors\": [...]}. Same as `framework_tool --thermalget`.",
        annotations(title = "Thermal Thresholds", read_only_hint = true)
    )]
    async fn thermal_thresholds(&self) -> ToolResult {
        self.read(|ec| {
            power::get_thermal_thresholds(ec)
                .map(|sensors| Thresholds { sensors })
                .ok_or_else(|| "Failed to read thermal thresholds from the EC".to_string())
        })
        .await
    }

    #[tool(
        description = "Ambient light sensor in lux and accelerometers (lid and base, in raw units where 16384 is 1G) with the lid angle in degrees. Sensors the system doesn't have are null. Same as `framework_tool --sensors`.",
        annotations(title = "Sensors", read_only_hint = true)
    )]
    async fn sensors(&self) -> ToolResult {
        self.read(|ec| {
            power::get_sensors(ec).ok_or_else(|| "Failed to read sensors from the EC".to_string())
        })
        .await
    }

    #[tool(
        description = "EC switch positions: lid open, power button pressed, firmware write protect disabled, dedicated recovery switch. Same as `framework_tool --switches`.",
        annotations(title = "Switches", read_only_hint = true)
    )]
    async fn switches(&self) -> ToolResult {
        self.read(|ec| {
            power::get_switches(ec).ok_or_else(|| "Failed to read switches from the EC".to_string())
        })
        .await
    }

    #[tool(
        description = "Chassis intrusion: whether the case is currently open, has ever been opened, how often, and whether the coin cell was ever removed. Same as `framework_tool --intrusion`.",
        annotations(title = "Chassis Intrusion", read_only_hint = true)
    )]
    async fn chassis_intrusion(&self) -> ToolResult {
        self.read(|ec| ec.get_intrusion_status().map_err(ec_err))
            .await
    }

    #[tool(
        description = "Hardware privacy switches: whether microphone and camera are connected. Same as `framework_tool --privacy`.",
        annotations(title = "Privacy Switches", read_only_hint = true)
    )]
    async fn privacy_switches(&self) -> ToolResult {
        self.read(|ec| {
            ec.get_privacy_info()
                .map(|(microphone, camera)| PrivacySwitches {
                    microphone_connected: microphone,
                    camera_connected: camera,
                })
                .map_err(ec_err)
        })
        .await
    }

    #[tool(
        description = "Keyboard backlight brightness in percent. Same as `framework_tool --kblight`.",
        annotations(title = "Keyboard Backlight", read_only_hint = true)
    )]
    async fn keyboard_backlight(&self) -> ToolResult {
        self.read(|ec| {
            ec.get_keyboard_backlight()
                .map(|percent| json!({ "percent": percent }))
                .map_err(ec_err)
        })
        .await
    }

    #[tool(
        description = "USB-C port state from the PD controller, per port: connection state, PD contract, power/data role, negotiated voltage (mV), current (mA) and power (mW), CC polarity, EPR, whether it is the active charging port and DP alt mode. Port 0 is right back, 3 is left back. Ports that don't exist are left out. Returns {\"ports\": [...]}. Same as `framework_tool --pdports`.",
        annotations(title = "USB-C PD Ports", read_only_hint = true)
    )]
    async fn pd_ports(&self) -> ToolResult {
        self.read(|ec| {
            let mut ports = vec![];
            for port in power::get_cypd_pd_info(ec) {
                ports.push(port.map_err(ec_err)?);
            }
            Ok(Ports { ports })
        })
        .await
    }

    #[tool(
        description = "USB-C power info as the EC sees it (ChromeOS EC_CMD_USB_PD_POWER_INFO), per port: role, charging type, voltage now/max (mV), current limit/max (mA), dual role and max power (uW). Ports that failed to respond are null. Returns {\"ports\": [...]}. Same as `framework_tool --pdports-chromebook`.",
        annotations(title = "USB-C Power Info", read_only_hint = true)
    )]
    async fn pd_power_info(&self) -> ToolResult {
        self.read(|ec| {
            let ports: Vec<Option<_>> = power::get_pd_info(ec, 4)
                .into_iter()
                .map(|port| port.ok())
                .collect();
            Ok(Ports { ports })
        })
        .await
    }

    #[tool(
        description = "EC system info: which image runs (RO/RW), why the EC was last reset and its sysinfo flags, raw and decoded. Same as `framework_tool --sysinfo`.",
        annotations(title = "EC System Info", read_only_hint = true)
    )]
    async fn ec_sysinfo(&self) -> ToolResult {
        self.read(|ec| ec.get_sysinfo().map_err(ec_err)).await
    }

    #[tool(
        description = "EC uptime in ms, how often the EC reset the AP since it booted, EC reset flags and the most recent AP resets with cause. Same as `framework_tool --uptimeinfo`.",
        annotations(title = "EC Uptime", read_only_hint = true)
    )]
    async fn ec_uptime(&self) -> ToolResult {
        self.read(|ec| ec.get_uptime_info().map_err(ec_err)).await
    }

    #[tool(
        description = "Features the EC firmware supports, as raw bitmask and list of enabled feature names. Same as `framework_tool --features`.",
        annotations(title = "EC Features", read_only_hint = true)
    )]
    async fn ec_features(&self) -> ToolResult {
        self.read(|ec| ec.get_features().map_err(ec_err)).await
    }

    #[tool(
        description = "Input deck status: chassis closed, connected daughterboards with board ID (Laptop 12/13), deck state and touchpad presence, top row module positions and SLEEP# GPIO (Laptop 16). Same as `framework_tool --inputdeck`.",
        annotations(title = "Input Deck", read_only_hint = true)
    )]
    async fn inputdeck(&self) -> ToolResult {
        self.read(|ec| ec.get_inputdeck_status().map_err(ec_err))
            .await
    }

    #[tool(
        description = "Expansion bay status (Laptop 16 only): module enabled, fault, door closed, board type, serial number, PCIe config and vendor. Same as `framework_tool --expansion-bay`.",
        annotations(title = "Expansion Bay", read_only_hint = true)
    )]
    async fn expansion_bay(&self) -> ToolResult {
        self.read(|ec| ec.get_bay_status().map_err(ec_err)).await
    }

    #[tool(
        description = "Saved EC panic (crash) data as {\"panic\": ...}, null if the EC never panicked: architecture, flags, plausibility checks and for Cortex-M the exception, registers (r0-r12, sp, lr, pc as array index 0-15) and decoded fault names. Same as `framework_tool --panicinfo`.",
        annotations(title = "EC Panic Info", read_only_hint = true)
    )]
    async fn ec_panic_info(&self) -> ToolResult {
        self.read(|ec| {
            ec.get_panic_info()
                .map(|data| Panic {
                    panic: framework_lib::chromium_ec::panic::parse_panic_info(&data),
                })
                .map_err(ec_err)
        })
        .await
    }

    #[tool(
        description = "Everything at once for debugging: platform, versions, power, charge limit, thermal, thresholds, sensors, switches, chassis, privacy, keyboard backlight, PD ports, EC sysinfo/uptime/features, input deck, expansion bay (Laptop 16) and panic info. Sections that could not be read contain an \"error\" field. Call this first when diagnosing a problem.",
        annotations(title = "System Snapshot", read_only_hint = true)
    )]
    async fn system_snapshot(&self) -> ToolResult {
        self.read(|ec| Ok(snapshot(ec))).await
    }
}

/// Collect every read-only tool into one object
fn snapshot(ec: &CrosEc) -> Value {
    let mut snapshot = serde_json::Map::new();
    let mut add = |name: &str, value: Value| {
        snapshot.insert(name.to_string(), value);
    };

    add("platform", section(Ok(platform_info())));
    add("versions", section(Ok(versions(ec))));
    add("power", section(Ok(power::get_power_status(ec))));
    add(
        "charge_limit",
        section(
            ec.get_charge_limit()
                .map_err(ec_err)
                .map(|(min, max)| ChargeLimit {
                    min_percent: min,
                    max_percent: max,
                }),
        ),
    );
    add("thermal", section(power::get_thermal(ec).map_err(ec_err)));
    add(
        "thermal_thresholds",
        section(power::get_thermal_thresholds(ec).ok_or_else(|| "Failed to read".to_string())),
    );
    add(
        "sensors",
        section(power::get_sensors(ec).ok_or_else(|| "Failed to read".to_string())),
    );
    add(
        "switches",
        section(power::get_switches(ec).ok_or_else(|| "Failed to read".to_string())),
    );
    add(
        "chassis_intrusion",
        section(ec.get_intrusion_status().map_err(ec_err)),
    );
    add(
        "privacy_switches",
        section(
            ec.get_privacy_info()
                .map_err(ec_err)
                .map(|(microphone, camera)| PrivacySwitches {
                    microphone_connected: microphone,
                    camera_connected: camera,
                }),
        ),
    );
    add(
        "keyboard_backlight",
        section(
            ec.get_keyboard_backlight()
                .map_err(ec_err)
                .map(|percent| json!({ "percent": percent })),
        ),
    );
    add(
        "pd_ports",
        section(
            power::get_cypd_pd_info(ec)
                .into_iter()
                .map(|port| port.map_err(ec_err))
                .collect::<Result<Vec<_>, _>>(),
        ),
    );
    add(
        "pd_power_info",
        section(Ok(power::get_pd_info(ec, 4)
            .into_iter()
            .map(|port| port.ok())
            .collect::<Vec<Option<_>>>())),
    );
    add("ec_sysinfo", section(ec.get_sysinfo().map_err(ec_err)));
    add("ec_uptime", section(ec.get_uptime_info().map_err(ec_err)));
    add("ec_features", section(ec.get_features().map_err(ec_err)));
    add(
        "inputdeck",
        section(ec.get_inputdeck_status().map_err(ec_err)),
    );
    if smbios::get_family() == Some(PlatformFamily::Framework16) {
        add(
            "expansion_bay",
            section(ec.get_bay_status().map_err(ec_err)),
        );
    }
    add(
        "ec_panic_info",
        section(
            ec.get_panic_info()
                .map_err(ec_err)
                .map(|data| framework_lib::chromium_ec::panic::parse_panic_info(&data)),
        ),
    );

    Value::Object(snapshot)
}

#[tool_handler(
    name = "framework_mcp",
    instructions = "Read-only access to the hardware state of this Framework Computer system through its Embedded Controller: battery and charger, USB-C PD ports, temperatures and fans, sensors, switches, firmware versions and EC diagnostics. Start with system_snapshot for an overview, then use the specific tools to re-read values that change. Units are in the field names or tool descriptions (degrees Celsius, mV, mA, mW, percent). Values decoded from bitmasks come both raw and as names. Some tools only apply to certain models, e.g. expansion_bay to the Laptop 16. Every tool names the framework_tool command that shows the same data, so the user can reproduce it."
)]
impl ServerHandler for FrameworkServer {}
