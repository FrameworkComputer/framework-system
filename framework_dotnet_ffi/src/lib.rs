use std::collections::VecDeque;
use std::convert::TryFrom;
use std::ptr;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};

use framework_lib::chromium_ec::command::EcRequestRaw;
use framework_lib::chromium_ec::commands::{EcFeatureCode, EcRequestGetFeatures};
use framework_lib::chromium_ec::{
    CrosEc, CrosEcDriverType, EcCurrentImage, EcError, EcResponseStatus,
};
use framework_lib::power::{self, ThermalSensorStatus, FAN_SLOT_COUNT, THERMAL_SENSOR_COUNT};
use framework_lib::smbios;
use framework_lib::smbios::{Platform, PlatformFamily};

const BATTERY_TEXT_LEN: usize = 8;
const STORED_DEVICE_ERROR_LIMIT: usize = 64;
#[cfg(target_os = "linux")]
const CROS_EC_DEV_PATH: &str = "/dev/cros_ec";

static NEXT_DEVICE_ERROR_ID: AtomicI32 = AtomicI32::new(1);
static DEVICE_ERROR_MESSAGES: OnceLock<Mutex<VecDeque<(i32, String)>>> = OnceLock::new();

pub struct FrameworkEcHandle {
    ec: CrosEc,
    driver: FrameworkEcDriver,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkStatusCode {
    Success = 0,
    NullPointer = -1,
    InvalidArgument = -2,
    NoDriverAvailable = -3,
    UnsupportedDriver = -4,
    DeviceError = -5,
    EcResponse = -6,
    UnknownResponseCode = -7,
    DataUnavailable = -8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkStatusNoPayload {
    pub reserved: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkStatusInvalidFanIndexRecord {
    pub fan_index: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkStatusEcResponseRecord {
    pub response: FrameworkEcResponseDetail,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkStatusUnknownEcResponseCodeRecord {
    pub response_code: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkStatusDeviceErrorRecord {
    pub message_token: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union FrameworkStatusPayload {
    pub none: FrameworkStatusNoPayload,
    pub invalid_fan_index: FrameworkStatusInvalidFanIndexRecord,
    pub ec_response: FrameworkStatusEcResponseRecord,
    pub unknown_ec_response_code: FrameworkStatusUnknownEcResponseCodeRecord,
    pub device_error: FrameworkStatusDeviceErrorRecord,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkStatus {
    pub code: FrameworkStatusCode,
    pub payload: FrameworkStatusPayload,
}

impl FrameworkStatus {
    fn success() -> Self {
        Self::no_payload(FrameworkStatusCode::Success)
    }

    fn with(code: FrameworkStatusCode, detail: i32) -> Self {
        match code {
            FrameworkStatusCode::Success => Self::success(),
            FrameworkStatusCode::NullPointer => Self::no_payload(FrameworkStatusCode::NullPointer),
            FrameworkStatusCode::InvalidArgument => Self::invalid_fan_index(detail),
            FrameworkStatusCode::NoDriverAvailable => {
                Self::no_payload(FrameworkStatusCode::NoDriverAvailable)
            }
            FrameworkStatusCode::UnsupportedDriver => {
                Self::no_payload(FrameworkStatusCode::UnsupportedDriver)
            }
            FrameworkStatusCode::DeviceError => Self::device_error(detail),
            FrameworkStatusCode::EcResponse => {
                Self::ec_response(ec_response_detail_from_raw(detail))
            }
            FrameworkStatusCode::UnknownResponseCode => Self::unknown_response_code(detail),
            FrameworkStatusCode::DataUnavailable => {
                Self::no_payload(FrameworkStatusCode::DataUnavailable)
            }
        }
    }

    fn no_payload(code: FrameworkStatusCode) -> Self {
        Self {
            code,
            payload: FrameworkStatusPayload {
                none: FrameworkStatusNoPayload { reserved: 0 },
            },
        }
    }

    fn invalid_fan_index(fan_index: i32) -> Self {
        Self {
            code: FrameworkStatusCode::InvalidArgument,
            payload: FrameworkStatusPayload {
                invalid_fan_index: FrameworkStatusInvalidFanIndexRecord { fan_index },
            },
        }
    }

    fn ec_response(response: FrameworkEcResponseDetail) -> Self {
        Self {
            code: FrameworkStatusCode::EcResponse,
            payload: FrameworkStatusPayload {
                ec_response: FrameworkStatusEcResponseRecord { response },
            },
        }
    }

    fn unknown_response_code(response_code: i32) -> Self {
        Self {
            code: FrameworkStatusCode::UnknownResponseCode,
            payload: FrameworkStatusPayload {
                unknown_ec_response_code: FrameworkStatusUnknownEcResponseCodeRecord {
                    response_code,
                },
            },
        }
    }

    fn device_error(message_token: i32) -> Self {
        Self {
            code: FrameworkStatusCode::DeviceError,
            payload: FrameworkStatusPayload {
                device_error: FrameworkStatusDeviceErrorRecord { message_token },
            },
        }
    }

    fn invalid_fan_index_value(&self) -> Option<i32> {
        if self.code != FrameworkStatusCode::InvalidArgument {
            return None;
        }

        Some(unsafe { self.payload.invalid_fan_index.fan_index })
    }

    fn ec_response_detail(&self) -> Option<FrameworkEcResponseDetail> {
        if self.code != FrameworkStatusCode::EcResponse {
            return None;
        }

        Some(unsafe { self.payload.ec_response.response })
    }

    fn unknown_response_code_value(&self) -> Option<i32> {
        if self.code != FrameworkStatusCode::UnknownResponseCode {
            return None;
        }

        Some(unsafe { self.payload.unknown_ec_response_code.response_code })
    }

    fn device_error_message_token(&self) -> Option<i32> {
        if self.code != FrameworkStatusCode::DeviceError {
            return None;
        }

        Some(unsafe { self.payload.device_error.message_token })
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcResponseDetail {
    Unknown = -1,
    Success = 0,
    InvalidCommand = 1,
    Error = 2,
    InvalidParameter = 3,
    AccessDenied = 4,
    InvalidResponse = 5,
    InvalidVersion = 6,
    InvalidChecksum = 7,
    InProgress = 8,
    Unavailable = 9,
    Timeout = 10,
    Overflow = 11,
    InvalidHeader = 12,
    RequestTruncated = 13,
    ResponseTooBig = 14,
    BusError = 15,
    Busy = 16,
}

impl From<EcResponseStatus> for FrameworkEcResponseDetail {
    fn from(value: EcResponseStatus) -> Self {
        match value {
            EcResponseStatus::Success => FrameworkEcResponseDetail::Success,
            EcResponseStatus::InvalidCommand => FrameworkEcResponseDetail::InvalidCommand,
            EcResponseStatus::Error => FrameworkEcResponseDetail::Error,
            EcResponseStatus::InvalidParameter => FrameworkEcResponseDetail::InvalidParameter,
            EcResponseStatus::AccessDenied => FrameworkEcResponseDetail::AccessDenied,
            EcResponseStatus::InvalidResponse => FrameworkEcResponseDetail::InvalidResponse,
            EcResponseStatus::InvalidVersion => FrameworkEcResponseDetail::InvalidVersion,
            EcResponseStatus::InvalidChecksum => FrameworkEcResponseDetail::InvalidChecksum,
            EcResponseStatus::InProgress => FrameworkEcResponseDetail::InProgress,
            EcResponseStatus::Unavailable => FrameworkEcResponseDetail::Unavailable,
            EcResponseStatus::Timeout => FrameworkEcResponseDetail::Timeout,
            EcResponseStatus::Overflow => FrameworkEcResponseDetail::Overflow,
            EcResponseStatus::InvalidHeader => FrameworkEcResponseDetail::InvalidHeader,
            EcResponseStatus::RequestTruncated => FrameworkEcResponseDetail::RequestTruncated,
            EcResponseStatus::ResponseTooBig => FrameworkEcResponseDetail::ResponseTooBig,
            EcResponseStatus::BusError => FrameworkEcResponseDetail::BusError,
            EcResponseStatus::Busy => FrameworkEcResponseDetail::Busy,
        }
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcDriver {
    Unknown = -1,
    Portio = 0,
    CrosEc = 1,
    Windows = 2,
}

impl From<CrosEcDriverType> for FrameworkEcDriver {
    fn from(value: CrosEcDriverType) -> Self {
        match value {
            CrosEcDriverType::Portio => FrameworkEcDriver::Portio,
            CrosEcDriverType::CrosEc => FrameworkEcDriver::CrosEc,
            CrosEcDriverType::Windows => FrameworkEcDriver::Windows,
        }
    }
}

impl TryFrom<FrameworkEcDriver> for CrosEcDriverType {
    type Error = ();

    fn try_from(value: FrameworkEcDriver) -> Result<Self, Self::Error> {
        match value {
            FrameworkEcDriver::Unknown => Err(()),
            FrameworkEcDriver::Portio => Ok(CrosEcDriverType::Portio),
            FrameworkEcDriver::CrosEc => Ok(CrosEcDriverType::CrosEc),
            FrameworkEcDriver::Windows => Ok(CrosEcDriverType::Windows),
        }
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkPlatform {
    Framework12IntelGen13 = 0,
    IntelGen11 = 1,
    IntelGen12 = 2,
    IntelGen13 = 3,
    IntelCoreUltra1 = 4,
    Framework13Amd7080 = 5,
    Framework13AmdAi300 = 6,
    Framework16Amd7080 = 7,
    Framework16AmdAi300 = 8,
    FrameworkDesktopAmdAiMax300 = 9,
    GenericFramework = 10,
    UnknownSystem = 11,
}

impl From<Platform> for FrameworkPlatform {
    fn from(value: Platform) -> Self {
        match value {
            Platform::Framework12IntelGen13 => FrameworkPlatform::Framework12IntelGen13,
            Platform::IntelGen11 => FrameworkPlatform::IntelGen11,
            Platform::IntelGen12 => FrameworkPlatform::IntelGen12,
            Platform::IntelGen13 => FrameworkPlatform::IntelGen13,
            Platform::IntelCoreUltra1 => FrameworkPlatform::IntelCoreUltra1,
            Platform::Framework13Amd7080 => FrameworkPlatform::Framework13Amd7080,
            Platform::Framework13AmdAi300 => FrameworkPlatform::Framework13AmdAi300,
            Platform::Framework16Amd7080 => FrameworkPlatform::Framework16Amd7080,
            Platform::Framework16AmdAi300 => FrameworkPlatform::Framework16AmdAi300,
            Platform::FrameworkDesktopAmdAiMax300 => FrameworkPlatform::FrameworkDesktopAmdAiMax300,
            Platform::GenericFramework(..) => FrameworkPlatform::GenericFramework,
            Platform::UnknownSystem => FrameworkPlatform::UnknownSystem,
        }
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkPlatformFamily {
    Unknown = -1,
    Framework12 = 0,
    Framework13 = 1,
    Framework16 = 2,
    FrameworkDesktop = 3,
}

impl From<PlatformFamily> for FrameworkPlatformFamily {
    fn from(value: PlatformFamily) -> Self {
        match value {
            PlatformFamily::Framework12 => FrameworkPlatformFamily::Framework12,
            PlatformFamily::Framework13 => FrameworkPlatformFamily::Framework13,
            PlatformFamily::Framework16 => FrameworkPlatformFamily::Framework16,
            PlatformFamily::FrameworkDesktop => FrameworkPlatformFamily::FrameworkDesktop,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkPlatformResult {
    pub status: FrameworkStatus,
    pub platform: FrameworkPlatform,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkPlatformFamilyResult {
    pub status: FrameworkStatus,
    pub family: FrameworkPlatformFamily,
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkTemperatureState {
    Ok = 0,
    NotPresent = 1,
    Error = 2,
    NotPowered = 3,
    NotCalibrated = 4,
}

impl From<ThermalSensorStatus> for FrameworkTemperatureState {
    fn from(value: ThermalSensorStatus) -> Self {
        match value {
            ThermalSensorStatus::Ok => FrameworkTemperatureState::Ok,
            ThermalSensorStatus::NotPresent => FrameworkTemperatureState::NotPresent,
            ThermalSensorStatus::Error => FrameworkTemperatureState::Error,
            ThermalSensorStatus::NotPowered => FrameworkTemperatureState::NotPowered,
            ThermalSensorStatus::NotCalibrated => FrameworkTemperatureState::NotCalibrated,
        }
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcCurrentImage {
    Unknown = 0,
    Ro = 1,
    Rw = 2,
}

impl From<EcCurrentImage> for FrameworkEcCurrentImage {
    fn from(value: EcCurrentImage) -> Self {
        match value {
            EcCurrentImage::Unknown => FrameworkEcCurrentImage::Unknown,
            EcCurrentImage::RO => FrameworkEcCurrentImage::Ro,
            EcCurrentImage::RW => FrameworkEcCurrentImage::Rw,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkTemperatureReading {
    pub state: FrameworkTemperatureState,
    pub celsius: i16,
    pub reserved: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkThermalSnapshot {
    pub fan_count: u8,
    pub reserved: [u8; 3],
    pub temperature_0: FrameworkTemperatureReading,
    pub temperature_1: FrameworkTemperatureReading,
    pub temperature_2: FrameworkTemperatureReading,
    pub temperature_3: FrameworkTemperatureReading,
    pub temperature_4: FrameworkTemperatureReading,
    pub temperature_5: FrameworkTemperatureReading,
    pub temperature_6: FrameworkTemperatureReading,
    pub temperature_7: FrameworkTemperatureReading,
    pub fan_rpm_0: u16,
    pub fan_rpm_1: u16,
    pub fan_rpm_2: u16,
    pub fan_rpm_3: u16,
    pub fan_present_0: u8,
    pub fan_present_1: u8,
    pub fan_present_2: u8,
    pub fan_present_3: u8,
    pub fan_stalled_0: u8,
    pub fan_stalled_1: u8,
    pub fan_stalled_2: u8,
    pub fan_stalled_3: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkFanCapabilities {
    pub fan_count: u8,
    pub supports_fan_control: u8,
    pub supports_thermal_reporting: u8,
    pub reserved: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkPowerSnapshot {
    pub ac_present: u8,
    pub battery_present: u8,
    pub discharging: u8,
    pub charging: u8,
    pub level_critical: u8,
    pub battery_count: u8,
    pub current_battery_index: u8,
    pub reserved: u8,
    pub present_voltage: u32,
    pub present_rate: u32,
    pub remaining_capacity: u32,
    pub design_capacity: u32,
    pub design_voltage: u32,
    pub last_full_charge_capacity: u32,
    pub cycle_count: u32,
    pub charge_percentage: u32,
    pub manufacturer: [u8; 8],
    pub model_number: [u8; 8],
    pub serial_number: [u8; 8],
    pub battery_type: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameworkEcFlashVersions {
    pub current_image: FrameworkEcCurrentImage,
    pub ro_version: [u8; 32],
    pub rw_version: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FrameworkByteBuffer {
    pub ptr: *mut u8,
    pub length: i32,
    pub capacity: i32,
}

impl Default for FrameworkByteBuffer {
    fn default() -> Self {
        Self {
            ptr: ptr::null_mut(),
            length: 0,
            capacity: 0,
        }
    }
}

impl FrameworkByteBuffer {
    fn from_vec(bytes: Vec<u8>) -> Self {
        let length = i32::try_from(bytes.len()).expect("buffer length overflowed i32");
        let capacity = i32::try_from(bytes.capacity()).expect("buffer capacity overflowed i32");
        let mut bytes = std::mem::ManuallyDrop::new(bytes);

        Self {
            ptr: bytes.as_mut_ptr(),
            length,
            capacity,
        }
    }

    unsafe fn destroy(self) {
        if self.ptr.is_null() {
            return;
        }

        let length = usize::try_from(self.length).expect("negative buffer length");
        let capacity = usize::try_from(self.capacity).expect("negative buffer capacity");
        drop(Vec::from_raw_parts(self.ptr, length, capacity));
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcHandleResult {
    pub status: FrameworkStatus,
    pub handle: *mut FrameworkEcHandle,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkProductNameResult {
    pub status: FrameworkStatus,
    pub product_name: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcBuildInfoResult {
    pub status: FrameworkStatus,
    pub build_info: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcFlashVersionsResult {
    pub status: FrameworkStatus,
    pub versions: FrameworkEcFlashVersions,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcPowerSnapshotResult {
    pub status: FrameworkStatus,
    pub snapshot: FrameworkPowerSnapshot,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcFanCapabilitiesResult {
    pub status: FrameworkStatus,
    pub capabilities: FrameworkFanCapabilities,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcThermalSnapshotResult {
    pub status: FrameworkStatus,
    pub snapshot: FrameworkThermalSnapshot,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcActiveDriverResult {
    pub status: FrameworkStatus,
    pub driver: FrameworkEcDriver,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkStatusDeviceErrorMessageResult {
    pub status: FrameworkStatus,
    pub message: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkStatusDescriptionResult {
    pub status: FrameworkStatus,
    pub description: FrameworkByteBuffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcSetFanRpmResult {
    pub status: FrameworkStatus,
    pub fan_index: i32,
    pub rpm: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcSetFanDutyResult {
    pub status: FrameworkStatus,
    pub fan_index: i32,
    pub percent: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FrameworkEcRestoreAutoFanControlResult {
    pub status: FrameworkStatus,
    pub fan_index: i32,
}

fn copy_text_bytes<const N: usize>(text: &str) -> [u8; N] {
    let mut buffer = [0u8; N];
    let bytes = text.as_bytes();
    let len = bytes.len().min(N);
    buffer[..len].copy_from_slice(&bytes[..len]);
    buffer
}

fn device_error_messages() -> &'static Mutex<VecDeque<(i32, String)>> {
    DEVICE_ERROR_MESSAGES.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn store_device_error_message(message: String) -> i32 {
    let id = NEXT_DEVICE_ERROR_ID.fetch_add(1, Ordering::Relaxed);
    let mut messages = device_error_messages()
        .lock()
        .expect("device error message lock poisoned");
    messages.push_back((id, message));
    while messages.len() > STORED_DEVICE_ERROR_LIMIT {
        messages.pop_front();
    }
    id
}

fn get_device_error_message(detail: i32) -> Option<String> {
    if detail <= 0 {
        return None;
    }

    let messages = device_error_messages().lock().ok()?;
    messages
        .iter()
        .find(|(id, _)| *id == detail)
        .map(|(_, message)| message.clone())
}

fn ec_response_detail_from_raw(detail: i32) -> FrameworkEcResponseDetail {
    match detail {
        0 => FrameworkEcResponseDetail::Success,
        1 => FrameworkEcResponseDetail::InvalidCommand,
        2 => FrameworkEcResponseDetail::Error,
        3 => FrameworkEcResponseDetail::InvalidParameter,
        4 => FrameworkEcResponseDetail::AccessDenied,
        5 => FrameworkEcResponseDetail::InvalidResponse,
        6 => FrameworkEcResponseDetail::InvalidVersion,
        7 => FrameworkEcResponseDetail::InvalidChecksum,
        8 => FrameworkEcResponseDetail::InProgress,
        9 => FrameworkEcResponseDetail::Unavailable,
        10 => FrameworkEcResponseDetail::Timeout,
        11 => FrameworkEcResponseDetail::Overflow,
        12 => FrameworkEcResponseDetail::InvalidHeader,
        13 => FrameworkEcResponseDetail::RequestTruncated,
        14 => FrameworkEcResponseDetail::ResponseTooBig,
        15 => FrameworkEcResponseDetail::BusError,
        16 => FrameworkEcResponseDetail::Busy,
        _ => FrameworkEcResponseDetail::Unknown,
    }
}
fn ec_response_detail_name(detail: FrameworkEcResponseDetail) -> &'static str {
    match detail {
        FrameworkEcResponseDetail::Unknown => "Unknown",
        FrameworkEcResponseDetail::Success => "Success",
        FrameworkEcResponseDetail::InvalidCommand => "InvalidCommand",
        FrameworkEcResponseDetail::Error => "Error",
        FrameworkEcResponseDetail::InvalidParameter => "InvalidParameter",
        FrameworkEcResponseDetail::AccessDenied => "AccessDenied",
        FrameworkEcResponseDetail::InvalidResponse => "InvalidResponse",
        FrameworkEcResponseDetail::InvalidVersion => "InvalidVersion",
        FrameworkEcResponseDetail::InvalidChecksum => "InvalidChecksum",
        FrameworkEcResponseDetail::InProgress => "InProgress",
        FrameworkEcResponseDetail::Unavailable => "Unavailable",
        FrameworkEcResponseDetail::Timeout => "Timeout",
        FrameworkEcResponseDetail::Overflow => "Overflow",
        FrameworkEcResponseDetail::InvalidHeader => "InvalidHeader",
        FrameworkEcResponseDetail::RequestTruncated => "RequestTruncated",
        FrameworkEcResponseDetail::ResponseTooBig => "ResponseTooBig",
        FrameworkEcResponseDetail::BusError => "BusError",
        FrameworkEcResponseDetail::Busy => "Busy",
    }
}

fn default_ec_flash_versions() -> FrameworkEcFlashVersions {
    FrameworkEcFlashVersions {
        current_image: FrameworkEcCurrentImage::Unknown,
        ro_version: [0; 32],
        rw_version: [0; 32],
    }
}

fn default_power_snapshot() -> FrameworkPowerSnapshot {
    FrameworkPowerSnapshot {
        ac_present: 0,
        battery_present: 0,
        discharging: 0,
        charging: 0,
        level_critical: 0,
        battery_count: 0,
        current_battery_index: 0,
        reserved: 0,
        present_voltage: 0,
        present_rate: 0,
        remaining_capacity: 0,
        design_capacity: 0,
        design_voltage: 0,
        last_full_charge_capacity: 0,
        cycle_count: 0,
        charge_percentage: 0,
        manufacturer: [0; BATTERY_TEXT_LEN],
        model_number: [0; BATTERY_TEXT_LEN],
        serial_number: [0; BATTERY_TEXT_LEN],
        battery_type: [0; BATTERY_TEXT_LEN],
    }
}

fn default_fan_capabilities() -> FrameworkFanCapabilities {
    FrameworkFanCapabilities {
        fan_count: 0,
        supports_fan_control: 0,
        supports_thermal_reporting: 0,
        reserved: 0,
    }
}

fn default_temperature_reading() -> FrameworkTemperatureReading {
    FrameworkTemperatureReading {
        state: FrameworkTemperatureState::NotPresent,
        celsius: 0,
        reserved: 0,
    }
}

fn default_thermal_snapshot() -> FrameworkThermalSnapshot {
    let reading = default_temperature_reading();

    FrameworkThermalSnapshot {
        fan_count: 0,
        reserved: [0; 3],
        temperature_0: reading,
        temperature_1: reading,
        temperature_2: reading,
        temperature_3: reading,
        temperature_4: reading,
        temperature_5: reading,
        temperature_6: reading,
        temperature_7: reading,
        fan_rpm_0: 0,
        fan_rpm_1: 0,
        fan_rpm_2: 0,
        fan_rpm_3: 0,
        fan_present_0: 0,
        fan_present_1: 0,
        fan_present_2: 0,
        fan_present_3: 0,
        fan_stalled_0: 0,
        fan_stalled_1: 0,
        fan_stalled_2: 0,
        fan_stalled_3: 0,
    }
}

fn ec_handle_result(
    status: FrameworkStatus,
    handle: *mut FrameworkEcHandle,
) -> FrameworkEcHandleResult {
    FrameworkEcHandleResult { status, handle }
}

fn product_name_result(
    status: FrameworkStatus,
    product_name: FrameworkByteBuffer,
) -> FrameworkProductNameResult {
    FrameworkProductNameResult {
        status,
        product_name,
    }
}

fn build_info_result(
    status: FrameworkStatus,
    build_info: FrameworkByteBuffer,
) -> FrameworkEcBuildInfoResult {
    FrameworkEcBuildInfoResult { status, build_info }
}

fn flash_versions_result(
    status: FrameworkStatus,
    versions: FrameworkEcFlashVersions,
) -> FrameworkEcFlashVersionsResult {
    FrameworkEcFlashVersionsResult { status, versions }
}

fn power_snapshot_result(
    status: FrameworkStatus,
    snapshot: FrameworkPowerSnapshot,
) -> FrameworkEcPowerSnapshotResult {
    FrameworkEcPowerSnapshotResult { status, snapshot }
}

fn fan_capabilities_result(
    status: FrameworkStatus,
    capabilities: FrameworkFanCapabilities,
) -> FrameworkEcFanCapabilitiesResult {
    FrameworkEcFanCapabilitiesResult {
        status,
        capabilities,
    }
}

fn thermal_snapshot_result(
    status: FrameworkStatus,
    snapshot: FrameworkThermalSnapshot,
) -> FrameworkEcThermalSnapshotResult {
    FrameworkEcThermalSnapshotResult { status, snapshot }
}

fn active_driver_result(
    status: FrameworkStatus,
    driver: FrameworkEcDriver,
) -> FrameworkEcActiveDriverResult {
    FrameworkEcActiveDriverResult { status, driver }
}

fn status_device_error_message_result(
    status: FrameworkStatus,
    message: FrameworkByteBuffer,
) -> FrameworkStatusDeviceErrorMessageResult {
    FrameworkStatusDeviceErrorMessageResult { status, message }
}

fn status_description_result(
    status: FrameworkStatus,
    description: FrameworkByteBuffer,
) -> FrameworkStatusDescriptionResult {
    FrameworkStatusDescriptionResult {
        status,
        description,
    }
}

fn platform_result(
    status: FrameworkStatus,
    platform: FrameworkPlatform,
) -> FrameworkPlatformResult {
    FrameworkPlatformResult { status, platform }
}

fn platform_family_result(
    status: FrameworkStatus,
    family: FrameworkPlatformFamily,
) -> FrameworkPlatformFamilyResult {
    FrameworkPlatformFamilyResult { status, family }
}

fn set_fan_rpm_result(
    status: FrameworkStatus,
    fan_index: i32,
    rpm: u32,
) -> FrameworkEcSetFanRpmResult {
    FrameworkEcSetFanRpmResult {
        status,
        fan_index,
        rpm,
    }
}

fn set_fan_duty_result(
    status: FrameworkStatus,
    fan_index: i32,
    percent: u32,
) -> FrameworkEcSetFanDutyResult {
    FrameworkEcSetFanDutyResult {
        status,
        fan_index,
        percent,
    }
}

fn restore_auto_fan_control_result(
    status: FrameworkStatus,
    fan_index: i32,
) -> FrameworkEcRestoreAutoFanControlResult {
    FrameworkEcRestoreAutoFanControlResult { status, fan_index }
}

fn status_description(status: FrameworkStatus) -> String {
    match status.code {
        FrameworkStatusCode::Success => "Success".to_string(),
        FrameworkStatusCode::NullPointer => "Null pointer".to_string(),
        FrameworkStatusCode::InvalidArgument => {
            format!(
                "Invalid fan index: {}",
                status.invalid_fan_index_value().unwrap_or_default()
            )
        }
        FrameworkStatusCode::NoDriverAvailable => "No EC driver available".to_string(),
        FrameworkStatusCode::UnsupportedDriver => {
            "Requested EC driver is not supported on this system".to_string()
        }
        FrameworkStatusCode::DeviceError => {
            if let Some(message) = status
                .device_error_message_token()
                .and_then(get_device_error_message)
            {
                format!("Device error: {}", message)
            } else {
                "Device error".to_string()
            }
        }
        FrameworkStatusCode::EcResponse => {
            let detail = status
                .ec_response_detail()
                .unwrap_or(FrameworkEcResponseDetail::Unknown);
            format!(
                "EC response: {} ({})",
                ec_response_detail_name(detail),
                detail as i32
            )
        }
        FrameworkStatusCode::UnknownResponseCode => {
            format!(
                "Unknown EC response code: {}",
                status.unknown_response_code_value().unwrap_or_default()
            )
        }
        FrameworkStatusCode::DataUnavailable => "Data unavailable".to_string(),
    }
}

fn status_from_error(error: EcError) -> FrameworkStatus {
    match error {
        EcError::Response(response) => {
            FrameworkStatus::with(FrameworkStatusCode::EcResponse, response as i32)
        }
        EcError::UnknownResponseCode(code) => FrameworkStatus::with(
            FrameworkStatusCode::UnknownResponseCode,
            i32::try_from(code).unwrap_or(i32::MAX),
        ),
        EcError::DeviceError(message) => {
            let detail = store_device_error_message(message);
            FrameworkStatus::with(FrameworkStatusCode::DeviceError, detail)
        }
    }
}

fn default_ec_handle() -> Option<FrameworkEcHandle> {
    #[cfg(windows)]
    if let Some(ec) = CrosEc::with(CrosEcDriverType::Windows) {
        return Some(FrameworkEcHandle {
            ec,
            driver: FrameworkEcDriver::Windows,
        });
    }

    #[cfg(target_os = "linux")]
    if std::path::Path::new(CROS_EC_DEV_PATH).exists() {
        if let Some(ec) = CrosEc::with(CrosEcDriverType::CrosEc) {
            return Some(FrameworkEcHandle {
                ec,
                driver: FrameworkEcDriver::CrosEc,
            });
        }
    }

    #[cfg(all(not(windows), target_arch = "x86_64"))]
    if let Some(ec) = CrosEc::with(CrosEcDriverType::Portio) {
        return Some(FrameworkEcHandle {
            ec,
            driver: FrameworkEcDriver::Portio,
        });
    }

    None
}

fn read_feature_flags(ec: &CrosEc) -> Result<[u32; 2], FrameworkStatus> {
    EcRequestGetFeatures {}
        .send_command(ec)
        .map(|response| response.flags)
        .map_err(status_from_error)
}

fn feature_enabled(ec: &CrosEc, feature: EcFeatureCode) -> Result<bool, FrameworkStatus> {
    let flags = read_feature_flags(ec)?;
    let index = feature as usize;
    let word = index / 32;
    let bit = index % 32;
    Ok((flags[word] & (1 << bit)) != 0)
}

fn require_handle<'a>(
    handle: *const FrameworkEcHandle,
) -> Result<&'a FrameworkEcHandle, FrameworkStatus> {
    if handle.is_null() {
        return Err(FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0));
    }

    // SAFETY: the caller guarantees the handle pointer came from framework_ec_open_*.
    Ok(unsafe { &*handle })
}

fn parse_optional_fan_index(fan_index: i32) -> Result<Option<u32>, FrameworkStatus> {
    if fan_index == -1 {
        return Ok(None);
    }

    let fan_index = u32::try_from(fan_index)
        .map_err(|_| FrameworkStatus::with(FrameworkStatusCode::InvalidArgument, fan_index))?;
    Ok(Some(fan_index))
}

fn parse_optional_fan_index_u8(fan_index: i32) -> Result<Option<u8>, FrameworkStatus> {
    if fan_index == -1 {
        return Ok(None);
    }

    let fan_index = u8::try_from(fan_index)
        .map_err(|_| FrameworkStatus::with(FrameworkStatusCode::InvalidArgument, fan_index))?;
    Ok(Some(fan_index))
}

#[no_mangle]
pub extern "C" fn framework_ec_driver_is_supported(driver: FrameworkEcDriver) -> bool {
    let Ok(driver) = CrosEcDriverType::try_from(driver) else {
        return false;
    };

    CrosEc::with(driver).is_some()
}

#[no_mangle]
/// The returned `message` buffer must be released with
/// `framework_byte_buffer_free`.
pub extern "C" fn framework_status_get_device_error_message(
    status: FrameworkStatus,
) -> FrameworkStatusDeviceErrorMessageResult {
    let Some(message) = status
        .device_error_message_token()
        .and_then(get_device_error_message)
    else {
        return status_device_error_message_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            FrameworkByteBuffer::default(),
        );
    };

    status_device_error_message_result(
        FrameworkStatus::success(),
        FrameworkByteBuffer::from_vec(message.into_bytes()),
    )
}

#[no_mangle]
/// The returned `description` buffer must be released with
/// `framework_byte_buffer_free`.
pub extern "C" fn framework_status_get_description(
    status: FrameworkStatus,
) -> FrameworkStatusDescriptionResult {
    status_description_result(
        FrameworkStatus::success(),
        FrameworkByteBuffer::from_vec(status_description(status).into_bytes()),
    )
}

#[no_mangle]
pub extern "C" fn framework_ec_open_default() -> FrameworkEcHandleResult {
    let Some(handle) = default_ec_handle() else {
        return ec_handle_result(
            FrameworkStatus::with(FrameworkStatusCode::NoDriverAvailable, 0),
            ptr::null_mut(),
        );
    };

    if let Err(error) = handle.ec.check_mem_magic() {
        return ec_handle_result(status_from_error(error), ptr::null_mut());
    }

    ec_handle_result(FrameworkStatus::success(), Box::into_raw(Box::new(handle)))
}

#[no_mangle]
pub extern "C" fn framework_ec_open_with_driver(
    driver: FrameworkEcDriver,
) -> FrameworkEcHandleResult {
    let Ok(driver_type) = CrosEcDriverType::try_from(driver) else {
        return ec_handle_result(
            FrameworkStatus::with(FrameworkStatusCode::UnsupportedDriver, 0),
            ptr::null_mut(),
        );
    };

    let Some(ec) = CrosEc::with(driver_type) else {
        return ec_handle_result(
            FrameworkStatus::with(FrameworkStatusCode::UnsupportedDriver, 0),
            ptr::null_mut(),
        );
    };

    if let Err(error) = ec.check_mem_magic() {
        return ec_handle_result(status_from_error(error), ptr::null_mut());
    }

    ec_handle_result(
        FrameworkStatus::success(),
        Box::into_raw(Box::new(FrameworkEcHandle { ec, driver })),
    )
}

#[no_mangle]
/// # Safety
/// `handle` must either be null or be a pointer previously returned by one of
/// the `framework_ec_open_*` functions that has not already been freed.
pub unsafe extern "C" fn framework_ec_close(handle: *mut FrameworkEcHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_active_driver(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcActiveDriverResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return active_driver_result(status, FrameworkEcDriver::Unknown),
    };

    active_driver_result(FrameworkStatus::success(), handle.driver)
}

#[no_mangle]
pub extern "C" fn framework_get_platform() -> FrameworkPlatformResult {
    match smbios::get_platform() {
        Some(platform) => platform_result(FrameworkStatus::success(), platform.into()),
        None => platform_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            FrameworkPlatform::UnknownSystem,
        ),
    }
}

#[no_mangle]
pub extern "C" fn framework_get_platform_family() -> FrameworkPlatformFamilyResult {
    match smbios::get_family() {
        Some(family) => platform_family_result(FrameworkStatus::success(), family.into()),
        None => platform_family_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            FrameworkPlatformFamily::Unknown,
        ),
    }
}

#[no_mangle]
pub extern "C" fn framework_get_product_name() -> FrameworkProductNameResult {
    let Some(product_name) = smbios::get_product_name() else {
        return product_name_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            FrameworkByteBuffer::default(),
        );
    };

    product_name_result(
        FrameworkStatus::success(),
        FrameworkByteBuffer::from_vec(product_name.into_bytes()),
    )
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library. The returned
/// `build_info` buffer must be released with `framework_byte_buffer_free`.
pub unsafe extern "C" fn framework_ec_get_build_info(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcBuildInfoResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return build_info_result(status, FrameworkByteBuffer::default()),
    };

    match handle.ec.version_info() {
        Ok(build_info) => build_info_result(
            FrameworkStatus::success(),
            FrameworkByteBuffer::from_vec(build_info.into_bytes()),
        ),
        Err(error) => build_info_result(status_from_error(error), FrameworkByteBuffer::default()),
    }
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_flash_versions(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcFlashVersionsResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return flash_versions_result(status, default_ec_flash_versions()),
    };

    let Some((ro_version, rw_version, current_image)) = handle.ec.flash_version() else {
        return flash_versions_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            default_ec_flash_versions(),
        );
    };

    flash_versions_result(
        FrameworkStatus::success(),
        FrameworkEcFlashVersions {
            current_image: current_image.into(),
            ro_version: copy_text_bytes(&ro_version),
            rw_version: copy_text_bytes(&rw_version),
        },
    )
}

#[no_mangle]
/// # Safety
/// `buffer` must either be the default zeroed buffer or a buffer previously
/// returned by this library that has not already been freed.
pub unsafe extern "C" fn framework_byte_buffer_free(buffer: FrameworkByteBuffer) {
    buffer.destroy();
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_power_snapshot(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcPowerSnapshotResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return power_snapshot_result(status, default_power_snapshot()),
    };

    let Some(power_info) = power::power_info(&handle.ec) else {
        return power_snapshot_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            default_power_snapshot(),
        );
    };

    let mut snapshot = FrameworkPowerSnapshot {
        ac_present: u8::from(power_info.ac_present),
        ..default_power_snapshot()
    };

    if let Some(battery) = power_info.battery {
        snapshot.battery_present = 1;
        snapshot.discharging = u8::from(battery.discharging);
        snapshot.charging = u8::from(battery.charging);
        snapshot.level_critical = u8::from(battery.level_critical);
        snapshot.battery_count = battery.battery_count;
        snapshot.current_battery_index = battery.current_battery_index;
        snapshot.present_voltage = battery.present_voltage;
        snapshot.present_rate = battery.present_rate;
        snapshot.remaining_capacity = battery.remaining_capacity;
        snapshot.design_capacity = battery.design_capacity;
        snapshot.design_voltage = battery.design_voltage;
        snapshot.last_full_charge_capacity = battery.last_full_charge_capacity;
        snapshot.cycle_count = battery.cycle_count;
        snapshot.charge_percentage = battery.charge_percentage;
        snapshot.manufacturer = copy_text_bytes(&battery.manufacturer);
        snapshot.model_number = copy_text_bytes(&battery.model_number);
        snapshot.serial_number = copy_text_bytes(&battery.serial_number);
        snapshot.battery_type = copy_text_bytes(&battery.battery_type);
    }

    power_snapshot_result(FrameworkStatus::success(), snapshot)
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_fan_capabilities(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcFanCapabilitiesResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return fan_capabilities_result(status, default_fan_capabilities()),
    };

    let fan_control = match feature_enabled(&handle.ec, EcFeatureCode::PwmFan) {
        Ok(supported) => supported,
        Err(status) => return fan_capabilities_result(status, default_fan_capabilities()),
    };
    let thermal = match feature_enabled(&handle.ec, EcFeatureCode::Thermal) {
        Ok(supported) => supported,
        Err(status) => return fan_capabilities_result(status, default_fan_capabilities()),
    };

    let fan_count = power::thermal_snapshot(&handle.ec)
        .map(|snapshot| snapshot.fan_count)
        .unwrap_or(0);

    fan_capabilities_result(
        FrameworkStatus::success(),
        FrameworkFanCapabilities {
            fan_count,
            supports_fan_control: u8::from(fan_control),
            supports_thermal_reporting: u8::from(thermal),
            reserved: 0,
        },
    )
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_get_thermal_snapshot(
    handle: *const FrameworkEcHandle,
) -> FrameworkEcThermalSnapshotResult {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return thermal_snapshot_result(status, default_thermal_snapshot()),
    };

    let Some(snapshot) = power::thermal_snapshot(&handle.ec) else {
        return thermal_snapshot_result(
            FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0),
            default_thermal_snapshot(),
        );
    };

    let mut temperatures = [default_temperature_reading(); THERMAL_SENSOR_COUNT];
    for (index, reading) in snapshot.temperatures.iter().enumerate() {
        temperatures[index] = FrameworkTemperatureReading {
            state: reading.status.into(),
            celsius: reading.celsius,
            reserved: 0,
        };
    }

    let mut fan_present = [0u8; FAN_SLOT_COUNT];
    let mut fan_stalled = [0u8; FAN_SLOT_COUNT];
    for index in 0..FAN_SLOT_COUNT {
        fan_present[index] = u8::from(snapshot.fan_present[index]);
        fan_stalled[index] = u8::from(snapshot.fan_stalled[index]);
    }

    thermal_snapshot_result(
        FrameworkStatus::success(),
        FrameworkThermalSnapshot {
            fan_count: snapshot.fan_count,
            reserved: [0; 3],
            temperature_0: temperatures[0],
            temperature_1: temperatures[1],
            temperature_2: temperatures[2],
            temperature_3: temperatures[3],
            temperature_4: temperatures[4],
            temperature_5: temperatures[5],
            temperature_6: temperatures[6],
            temperature_7: temperatures[7],
            fan_rpm_0: snapshot.fan_rpms[0],
            fan_rpm_1: snapshot.fan_rpms[1],
            fan_rpm_2: snapshot.fan_rpms[2],
            fan_rpm_3: snapshot.fan_rpms[3],
            fan_present_0: fan_present[0],
            fan_present_1: fan_present[1],
            fan_present_2: fan_present[2],
            fan_present_3: fan_present[3],
            fan_stalled_0: fan_stalled[0],
            fan_stalled_1: fan_stalled[1],
            fan_stalled_2: fan_stalled[2],
            fan_stalled_3: fan_stalled[3],
        },
    )
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_set_fan_rpm(
    handle: *const FrameworkEcHandle,
    fan_index: i32,
    rpm: u32,
) -> FrameworkEcSetFanRpmResult {
    let requested_fan_index = fan_index;

    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return set_fan_rpm_result(status, fan_index, rpm),
    };
    let fan_index = match parse_optional_fan_index(fan_index) {
        Ok(fan_index) => fan_index,
        Err(status) => return set_fan_rpm_result(status, requested_fan_index, rpm),
    };

    let status = match handle.ec.fan_set_rpm(fan_index, rpm) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    };

    set_fan_rpm_result(status, requested_fan_index, rpm)
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_set_fan_duty(
    handle: *const FrameworkEcHandle,
    fan_index: i32,
    percent: u32,
) -> FrameworkEcSetFanDutyResult {
    let requested_fan_index = fan_index;

    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return set_fan_duty_result(status, fan_index, percent),
    };
    let fan_index = match parse_optional_fan_index(fan_index) {
        Ok(fan_index) => fan_index,
        Err(status) => return set_fan_duty_result(status, requested_fan_index, percent),
    };

    let status = match handle.ec.fan_set_duty(fan_index, percent) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    };

    set_fan_duty_result(status, requested_fan_index, percent)
}

#[no_mangle]
/// # Safety
/// `handle` must be a valid pointer returned by this library.
pub unsafe extern "C" fn framework_ec_restore_auto_fan_control(
    handle: *const FrameworkEcHandle,
    fan_index: i32,
) -> FrameworkEcRestoreAutoFanControlResult {
    let requested_fan_index = fan_index;

    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return restore_auto_fan_control_result(status, fan_index),
    };
    let fan_index = match parse_optional_fan_index_u8(fan_index) {
        Ok(fan_index) => fan_index,
        Err(status) => return restore_auto_fan_control_result(status, requested_fan_index),
    };

    let status = match handle.ec.autofanctrl(fan_index) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    };

    restore_auto_fan_control_result(status, requested_fan_index)
}
