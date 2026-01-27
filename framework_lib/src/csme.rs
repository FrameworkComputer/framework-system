//! Get CSME information from the running system
//!
//! Supports two methods:
//! - Linux sysfs: reads from /sys/class/mei
//! - SMBIOS type 0xDB: OEM table with HFSTS registers (works on any platform)

use alloc::string::ToString;
use alloc::vec::Vec;
use core::fmt;
#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::io;
#[cfg(target_os = "linux")]
use std::path::Path;

use smbioslib::{DefinedStruct, UndefinedStruct};

/// SMBIOS type for ME Firmware Status (FWSTS) table
pub const SMBIOS_TYPE_ME_FWSTS: u8 = 0xDB;

/// ME Family based on firmware version
/// TODO: Can we split them up?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeFamily {
    Unknown,
    Txe,    // Trusted Execution Engine (major 1-5)
    Me,     // Management Engine (major 6-10)
    Csme11, // Converged Security ME 11-15
    Csme16, // CSME 16-17
    Csme18, // CSME 18+
}

impl MeFamily {
    /// Determine ME family from major version number
    pub fn from_version(major: u32) -> Self {
        match major {
            0 => MeFamily::Unknown,
            1..=5 => MeFamily::Txe,
            6..=10 => MeFamily::Me,
            11..=15 => MeFamily::Csme11,
            16..=17 => MeFamily::Csme16,
            _ => MeFamily::Csme18,
        }
    }
}

impl fmt::Display for MeFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeFamily::Unknown => write!(f, "Unknown"),
            MeFamily::Txe => write!(f, "TXE"),
            MeFamily::Me => write!(f, "ME"),
            MeFamily::Csme11 => write!(f, "CSME 11-15"),
            MeFamily::Csme16 => write!(f, "CSME 16-17"),
            MeFamily::Csme18 => write!(f, "CSME 18+"),
        }
    }
}

/// Current Working State from HFSTS1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeWorkingState {
    Reset,
    Initializing,
    Recovery,
    Test,
    Disabled,
    Normal,
    Wait,
    Transition,
    InvalidCpu,
    Halt,
    Unknown(u8),
}

impl From<u8> for MeWorkingState {
    fn from(val: u8) -> Self {
        match val {
            0 => MeWorkingState::Reset,
            1 => MeWorkingState::Initializing,
            2 => MeWorkingState::Recovery,
            3 => MeWorkingState::Test,
            4 => MeWorkingState::Disabled,
            5 => MeWorkingState::Normal,
            6 => MeWorkingState::Wait,
            7 => MeWorkingState::Transition,
            8 => MeWorkingState::InvalidCpu,
            0x0E => MeWorkingState::Halt,
            v => MeWorkingState::Unknown(v),
        }
    }
}

impl fmt::Display for MeWorkingState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeWorkingState::Reset => write!(f, "Reset"),
            MeWorkingState::Initializing => write!(f, "Initializing"),
            MeWorkingState::Recovery => write!(f, "Recovery"),
            MeWorkingState::Test => write!(f, "Test"),
            MeWorkingState::Disabled => write!(f, "Disabled"),
            MeWorkingState::Normal => write!(f, "Normal"),
            MeWorkingState::Wait => write!(f, "Wait"),
            MeWorkingState::Transition => write!(f, "Transition"),
            MeWorkingState::InvalidCpu => write!(f, "Invalid CPU"),
            MeWorkingState::Halt => write!(f, "Halt"),
            MeWorkingState::Unknown(v) => write!(f, "Unknown(0x{:X})", v),
        }
    }
}

/// Operation Mode from HFSTS1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeOperationMode {
    Normal,
    Debug,
    Disable,
    OverrideJumper,
    OverrideMei,
    EnhancedDebug,
    Unknown(u8),
}

impl From<u8> for MeOperationMode {
    fn from(val: u8) -> Self {
        match val {
            0 => MeOperationMode::Normal,
            2 => MeOperationMode::Debug,
            3 => MeOperationMode::Disable,
            4 => MeOperationMode::OverrideJumper,
            5 => MeOperationMode::OverrideMei,
            7 => MeOperationMode::EnhancedDebug,
            v => MeOperationMode::Unknown(v),
        }
    }
}

impl fmt::Display for MeOperationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeOperationMode::Normal => write!(f, "Normal"),
            MeOperationMode::Debug => write!(f, "Debug"),
            MeOperationMode::Disable => write!(f, "Disabled"),
            MeOperationMode::OverrideJumper => write!(f, "Override (Jumper)"),
            MeOperationMode::OverrideMei => write!(f, "Override (MEI)"),
            MeOperationMode::EnhancedDebug => write!(f, "Enhanced Debug"),
            MeOperationMode::Unknown(v) => write!(f, "Unknown(0x{:X})", v),
        }
    }
}

/// Bootguard enforcement policy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootguardPolicy {
    Nothing,
    ShutdownTimeout,
    ShutdownNow,
    Shutdown30Mins,
    Unknown(u8),
}

impl From<u8> for BootguardPolicy {
    fn from(val: u8) -> Self {
        match val {
            0 => BootguardPolicy::Nothing,
            1 => BootguardPolicy::ShutdownTimeout,
            2 => BootguardPolicy::ShutdownNow,
            3 => BootguardPolicy::Shutdown30Mins,
            v => BootguardPolicy::Unknown(v),
        }
    }
}

impl fmt::Display for BootguardPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BootguardPolicy::Nothing => write!(f, "Do Nothing"),
            BootguardPolicy::ShutdownTimeout => write!(f, "Shutdown (Timeout)"),
            BootguardPolicy::ShutdownNow => write!(f, "Shutdown Immediately"),
            BootguardPolicy::Shutdown30Mins => write!(f, "Shutdown in 30 Minutes"),
            BootguardPolicy::Unknown(v) => write!(f, "Unknown(0x{:X})", v),
        }
    }
}

/// Bootguard status parsed from HFSTS registers
#[derive(Debug, Clone)]
pub struct BootguardStatus {
    /// Whether bootguard is enabled
    pub enabled: bool,
    /// Whether verified boot is active (CSME11-17)
    pub verified_boot: Option<bool>,
    /// Whether ACM (Authenticated Code Module) protection is active
    pub acm_active: bool,
    /// ACM execution completed successfully
    pub acm_done: Option<bool>,
    /// Enforcement policy on failure
    pub policy: Option<BootguardPolicy>,
    /// FPF (Field Programmable Fuses) SOC lock status
    pub fpf_soc_lock: bool,
}

impl fmt::Display for BootguardStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Enabled: {}", if self.enabled { "Yes" } else { "No" })?;
        if let Some(verified) = self.verified_boot {
            write!(
                f,
                ", Verified Boot: {}",
                if verified { "Yes" } else { "No" }
            )?;
        }
        write!(
            f,
            ", ACM Active: {}",
            if self.acm_active { "Yes" } else { "No" }
        )?;
        if let Some(done) = self.acm_done {
            write!(f, ", ACM Done: {}", if done { "Yes" } else { "No" })?;
        }
        if let Some(ref policy) = self.policy {
            write!(f, ", Policy: {}", policy)?;
        }
        write!(
            f,
            ", FPF Lock: {}",
            if self.fpf_soc_lock { "Yes" } else { "No" }
        )
    }
}

/// HFSTS (Host Firmware Status) registers from SMBIOS
#[derive(Debug, Clone)]
pub struct HfStsRegisters {
    pub hfsts1: u32,
    pub hfsts2: u32,
    pub hfsts3: u32,
    pub hfsts4: u32,
    pub hfsts5: u32,
    pub hfsts6: u32,
}

impl HfStsRegisters {
    /// Parse HFSTS1 to get working state (bits 0-3)
    pub fn working_state(&self) -> MeWorkingState {
        MeWorkingState::from((self.hfsts1 & 0x0F) as u8)
    }

    /// Parse HFSTS1 to get manufacturing mode (bit 4) - CSME11-15 only
    pub fn manufacturing_mode(&self) -> bool {
        (self.hfsts1 >> 4) & 1 == 1
    }

    /// Parse HFSTS1 to get SPI protection mode (bit 4) - CSME18+ only
    pub fn spi_protection_mode(&self) -> bool {
        (self.hfsts1 >> 4) & 1 == 1
    }

    /// Parse HFSTS1 to get operation mode (bits 16-19)
    pub fn operation_mode(&self) -> MeOperationMode {
        MeOperationMode::from(((self.hfsts1 >> 16) & 0x0F) as u8)
    }

    /// Parse bootguard status for CSME11-17 (from HFSTS6)
    pub fn bootguard_csme11(&self) -> BootguardStatus {
        let hfsts6 = self.hfsts6;

        // Bit 28: boot_guard_disable (inverted for "enabled")
        let boot_guard_disable = (hfsts6 >> 28) & 1 == 1;
        let enabled = !boot_guard_disable;

        // Bit 0: force_boot_guard_acm
        let acm_active = (hfsts6 & 1) == 1;

        // Bit 9: verified_boot
        let verified_boot = (hfsts6 >> 9) & 1 == 1;

        // Bits 6-7: error_enforce_policy
        let policy = BootguardPolicy::from(((hfsts6 >> 6) & 0x03) as u8);

        // Bit 30: fpf_soc_lock
        let fpf_soc_lock = (hfsts6 >> 30) & 1 == 1;

        BootguardStatus {
            enabled,
            verified_boot: Some(verified_boot),
            acm_active,
            acm_done: None,
            policy: Some(policy),
            fpf_soc_lock,
        }
    }

    /// Parse bootguard status for CSME18+ (from HFSTS5 and HFSTS6)
    pub fn bootguard_csme18(&self) -> BootguardStatus {
        let hfsts5 = self.hfsts5;
        let hfsts6 = self.hfsts6;

        // HFSTS5 bit 1: valid (bootguard enabled)
        let enabled = (hfsts5 >> 1) & 1 == 1;

        // HFSTS5 bit 0: btg_acm_active
        let acm_active = (hfsts5 & 1) == 1;

        // HFSTS5 bit 8: acm_done_sts
        let acm_done = (hfsts5 >> 8) & 1 == 1;

        // HFSTS6 bit 30: fpf_soc_configuration_lock
        let fpf_soc_lock = (hfsts6 >> 30) & 1 == 1;

        BootguardStatus {
            enabled,
            verified_boot: None, // Not available in CSME18 the same way
            acm_active,
            acm_done: Some(acm_done),
            policy: None, // Different structure in CSME18
            fpf_soc_lock,
        }
    }
}

/// ME component name in FWSTS record
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeComponent {
    Mei1, // PCI 0:22:0
    Mei2, // PCI 0:22:1
    Mei3, // PCI 0:22:4
    Mei4,
    Unknown(u8),
}

impl From<u8> for MeComponent {
    fn from(val: u8) -> Self {
        match val {
            1 => MeComponent::Mei1,
            2 => MeComponent::Mei2,
            3 => MeComponent::Mei3,
            4 => MeComponent::Mei4,
            v => MeComponent::Unknown(v),
        }
    }
}

/// ME FWSTS record from SMBIOS type 0xDB
#[derive(Debug, Clone)]
pub struct MeFwstsRecord {
    pub component: MeComponent,
    pub hfsts: HfStsRegisters,
}

/// ME information parsed from SMBIOS type 0xDB
#[derive(Debug, Clone)]
pub struct MeSmbiosInfo {
    pub handle: u16,
    pub version: u8,
    pub records: Vec<MeFwstsRecord>,
}

impl MeSmbiosInfo {
    /// Get the primary MEI1 record (most common)
    pub fn mei1(&self) -> Option<&MeFwstsRecord> {
        self.records
            .iter()
            .find(|r| r.component == MeComponent::Mei1)
    }

    /// Get bootguard status based on ME family
    pub fn bootguard_status(&self, family: MeFamily) -> Option<BootguardStatus> {
        let record = self.mei1()?;
        Some(match family {
            MeFamily::Csme11 | MeFamily::Csme16 => record.hfsts.bootguard_csme11(),
            MeFamily::Csme18 => record.hfsts.bootguard_csme18(),
            _ => return None, // Bootguard not supported on older ME
        })
    }

    /// Get working state from MEI1
    pub fn working_state(&self) -> Option<MeWorkingState> {
        Some(self.mei1()?.hfsts.working_state())
    }

    /// Get operation mode from MEI1
    pub fn operation_mode(&self) -> Option<MeOperationMode> {
        Some(self.mei1()?.hfsts.operation_mode())
    }
}

/// Parse SMBIOS type 0xDB (ME FWSTS) table
///
/// Structure:
/// - Offset 0: type (0xDB)
/// - Offset 1: length
/// - Offset 2-3: handle (u16le)
/// - Offset 4: version (should be 0x01)
/// - Offset 5: count (number of records)
/// - Offset 6+: records, each 25 bytes:
///   - 1 byte: component name
///   - 24 bytes: 6 x u32le HFSTS registers
pub fn parse_me_fwsts(undefined_struct: &UndefinedStruct) -> Option<MeSmbiosInfo> {
    // Verify this is type 0xDB
    if undefined_struct.header.struct_type() != SMBIOS_TYPE_ME_FWSTS {
        return None;
    }

    let length = undefined_struct.header.length() as usize;
    let handle = *undefined_struct.header.handle();

    // Minimum header size: 6 bytes (type, length, handle, version, count)
    if length < 6 {
        return None;
    }

    // Get version and count from offsets 4 and 5
    let version = undefined_struct.get_field_byte(4)?;
    let count = undefined_struct.get_field_byte(5)?;

    // Version should be 0x01
    if version != 0x01 {
        return None;
    }

    let mut records = Vec::new();
    let record_size = 25; // 1 byte component + 6 * 4 bytes HFSTS

    for i in 0..count {
        let record_offset = 6 + (i as usize * record_size);

        // Check we have enough data
        // TODO: Should this `return None;`?
        if record_offset + record_size > length {
            break;
        }

        let component = MeComponent::from(undefined_struct.get_field_byte(record_offset)?);

        // Parse 6 HFSTS registers (u32le each)
        let hfsts1 = undefined_struct.get_field_dword(record_offset + 1)?;
        let hfsts2 = undefined_struct.get_field_dword(record_offset + 5)?;
        let hfsts3 = undefined_struct.get_field_dword(record_offset + 9)?;
        let hfsts4 = undefined_struct.get_field_dword(record_offset + 13)?;
        let hfsts5 = undefined_struct.get_field_dword(record_offset + 17)?;
        let hfsts6 = undefined_struct.get_field_dword(record_offset + 21)?;

        records.push(MeFwstsRecord {
            component,
            hfsts: HfStsRegisters {
                hfsts1,
                hfsts2,
                hfsts3,
                hfsts4,
                hfsts5,
                hfsts6,
            },
        });
    }

    Some(MeSmbiosInfo {
        handle,
        version,
        records,
    })
}

/// SMBIOS type for ME Firmware Version Info (FVI) table
pub const SMBIOS_TYPE_ME_FVI: u8 = 0xDD;

/// Well-known handles for ME in SMBIOS 0xDD tables
pub const SMBIOS_DD_HANDLE_ME: u16 = 0x18;
pub const SMBIOS_DD_HANDLE_ME2: u16 = 0x30;

/// ME version parsed from SMBIOS type 0xDD (FVI)
#[derive(Debug, Clone)]
pub struct MeFviVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub build: u16,
}

impl fmt::Display for MeFviVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.major, self.minor, self.patch, self.build
        )
    }
}

/// Find ME-related handles from SMBIOS Type 14 (Group Associations)
///
/// Looks for Group Associations with "$MEI" or "Firmware Version Info" group name
/// and returns the handles they point to.
pub fn find_me_handles_from_type14(smbios: &smbioslib::SMBiosData) -> Vec<u16> {
    let mut handles = Vec::new();

    for undefined_struct in smbios.iter() {
        if let DefinedStruct::GroupAssociations(group) = undefined_struct.defined_struct() {
            // Check if this group is ME-related
            let group_name = group.group_name().to_string();
            if group_name.contains("$MEI") || group_name.contains("Firmware Version Info") {
                // Collect all handles from this group
                for item in group.item_iterator() {
                    if let Some(handle) = item.item_handle() {
                        handles.push(*handle);
                    }
                }
            }
        }
    }

    handles
}

/// Get ME version from SMBIOS type 0xDD (FVI) tables
pub fn me_version_from_smbios(smbios: &smbioslib::SMBiosData) -> Option<MeFviVersion> {
    // First try to find handles from Type 14
    let handles = find_me_handles_from_type14(smbios);

    // Find versions from components 1, 2, or 3 in valid ME tables
    // The component location varies between systems, so we find all candidates
    // and return the one with the highest major version (most likely the actual ME version)
    let mut best_version: Option<MeFviVersion> = None;

    for undefined_struct in smbios.iter() {
        if let Some(version) = parse_me_fvi_version(undefined_struct, &handles) {
            // Keep the version with the highest major number
            if best_version.is_none() || version.major > best_version.as_ref().unwrap().major {
                best_version = Some(version);
            }
        }
    }

    best_version
}

/// Parse ME version from FVI table
/// Returns the highest major version found from components 1, 2, or 3
/// (ME version is typically higher than reference code versions)
fn parse_me_fvi_version(
    undefined_struct: &UndefinedStruct,
    valid_handles: &[u16],
) -> Option<MeFviVersion> {
    if undefined_struct.header.struct_type() != SMBIOS_TYPE_ME_FVI {
        return None;
    }

    let handle = *undefined_struct.header.handle();
    let length = undefined_struct.header.length() as usize;

    let is_valid_handle = if valid_handles.is_empty() {
        handle == SMBIOS_DD_HANDLE_ME || handle == SMBIOS_DD_HANDLE_ME2
    } else {
        valid_handles.contains(&handle)
    };

    if !is_valid_handle {
        return None;
    }

    let count = undefined_struct.get_field_byte(4)?;
    if count == 0 {
        return None;
    }

    let record_size = 7;
    let records_offset = 5;

    let mut best_version: Option<MeFviVersion> = None;

    for i in 0..count {
        let offset = records_offset + (i as usize * record_size);
        if offset + record_size > length {
            break;
        }

        let component_name = undefined_struct.get_field_byte(offset)?;

        // Check components 1, 2, and 3 - ME version location varies by system
        if component_name == 1 || component_name == 2 || component_name == 3 {
            let major = undefined_struct.get_field_byte(offset + 2)?;
            let minor = undefined_struct.get_field_byte(offset + 3)?;
            let patch = undefined_struct.get_field_byte(offset + 4)?;
            let build = undefined_struct.get_field_word(offset + 5)?;

            // Skip invalid versions (all 0xFF)
            if major == 0xFF && minor == 0xFF && patch == 0xFF {
                continue;
            }

            // Keep the highest major version (ME version is typically higher than reference code)
            if best_version.is_none() || major > best_version.as_ref().unwrap().major {
                best_version = Some(MeFviVersion {
                    major,
                    minor,
                    patch,
                    build,
                });
            }
        }
    }

    best_version
}

/// Get ME FWSTS info from SMBIOS tables
pub fn me_fwsts_from_smbios(smbios: &smbioslib::SMBiosData) -> Option<MeSmbiosInfo> {
    // For type 0xDB, we look for any table with MEI1 component
    // (unlike 0xDD, the 0xDB table doesn't need handle validation from Type 14)
    for undefined_struct in smbios.iter() {
        if undefined_struct.header.struct_type() == SMBIOS_TYPE_ME_FWSTS {
            if let Some(info) = parse_me_fwsts(undefined_struct) {
                // Only return if we found a MEI1 record
                if info.mei1().is_some() {
                    return Some(info);
                }
            }
        }
    }
    None
}

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
