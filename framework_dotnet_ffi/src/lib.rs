use std::convert::TryFrom;
use std::ptr;

use framework_lib::chromium_ec::command::EcRequestRaw;
use framework_lib::chromium_ec::commands::{EcFeatureCode, EcRequestGetFeatures};
use framework_lib::chromium_ec::{CrosEc, CrosEcDriverType, EcCurrentImage, EcError};
use framework_lib::power::{self, ThermalSensorStatus, FAN_SLOT_COUNT, THERMAL_SENSOR_COUNT};
use framework_lib::smbios;
use framework_lib::smbios::{Platform, PlatformFamily};

const BATTERY_TEXT_LEN: usize = 8;
#[cfg(target_os = "linux")]
const CROS_EC_DEV_PATH: &str = "/dev/cros_ec";

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
pub struct FrameworkStatus {
    pub code: FrameworkStatusCode,
    pub detail: i32,
}

impl FrameworkStatus {
    fn success() -> Self {
        Self {
            code: FrameworkStatusCode::Success,
            detail: 0,
        }
    }

    fn with(code: FrameworkStatusCode, detail: i32) -> Self {
        Self { code, detail }
    }
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkEcDriver {
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

impl From<FrameworkEcDriver> for CrosEcDriverType {
    fn from(value: FrameworkEcDriver) -> Self {
        match value {
            FrameworkEcDriver::Portio => CrosEcDriverType::Portio,
            FrameworkEcDriver::CrosEc => CrosEcDriverType::CrosEc,
            FrameworkEcDriver::Windows => CrosEcDriverType::Windows,
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

fn copy_text_bytes<const N: usize>(text: &str) -> [u8; N] {
    let mut buffer = [0u8; N];
    let bytes = text.as_bytes();
    let len = bytes.len().min(N);
    buffer[..len].copy_from_slice(&bytes[..len]);
    buffer
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
        EcError::DeviceError(_) => FrameworkStatus::with(FrameworkStatusCode::DeviceError, 0),
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
    CrosEc::with(driver.into()).is_some()
}

#[no_mangle]
pub unsafe extern "C" fn framework_ec_open_default(
    out_handle: *mut *mut FrameworkEcHandle,
) -> FrameworkStatus {
    if out_handle.is_null() {
        return FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0);
    }

    let Some(handle) = default_ec_handle() else {
        *out_handle = ptr::null_mut();
        return FrameworkStatus::with(FrameworkStatusCode::NoDriverAvailable, 0);
    };

    if let Err(error) = handle.ec.check_mem_magic() {
        *out_handle = ptr::null_mut();
        return status_from_error(error);
    }

    *out_handle = Box::into_raw(Box::new(handle));
    FrameworkStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn framework_ec_open_with_driver(
    driver: FrameworkEcDriver,
    out_handle: *mut *mut FrameworkEcHandle,
) -> FrameworkStatus {
    if out_handle.is_null() {
        return FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0);
    }

    let Some(ec) = CrosEc::with(driver.into()) else {
        *out_handle = ptr::null_mut();
        return FrameworkStatus::with(FrameworkStatusCode::UnsupportedDriver, 0);
    };

    if let Err(error) = ec.check_mem_magic() {
        *out_handle = ptr::null_mut();
        return status_from_error(error);
    }

    *out_handle = Box::into_raw(Box::new(FrameworkEcHandle { ec, driver }));
    FrameworkStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn framework_ec_close(handle: *mut FrameworkEcHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

#[no_mangle]
pub unsafe extern "C" fn framework_ec_get_active_driver(
    handle: *const FrameworkEcHandle,
    out_driver: *mut FrameworkEcDriver,
) -> FrameworkStatus {
    if out_driver.is_null() {
        return FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0);
    }

    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };

    *out_driver = handle.driver;
    FrameworkStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn framework_get_platform(
    out_platform: *mut FrameworkPlatform,
) -> FrameworkStatus {
    if out_platform.is_null() {
        return FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0);
    }

    *out_platform = smbios::get_platform()
        .unwrap_or(Platform::UnknownSystem)
        .into();
    FrameworkStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn framework_get_platform_family(
    out_family: *mut FrameworkPlatformFamily,
) -> FrameworkStatus {
    if out_family.is_null() {
        return FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0);
    }

    *out_family = smbios::get_family()
        .map(FrameworkPlatformFamily::from)
        .unwrap_or(FrameworkPlatformFamily::Unknown);
    FrameworkStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn framework_get_product_name(
    out_buffer: *mut FrameworkByteBuffer,
) -> FrameworkStatus {
    if out_buffer.is_null() {
        return FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0);
    }

    let Some(product_name) = smbios::get_product_name() else {
        *out_buffer = FrameworkByteBuffer::default();
        return FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0);
    };

    *out_buffer = FrameworkByteBuffer::from_vec(product_name.into_bytes());
    FrameworkStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn framework_ec_get_build_info(
    handle: *const FrameworkEcHandle,
    out_buffer: *mut FrameworkByteBuffer,
) -> FrameworkStatus {
    if out_buffer.is_null() {
        return FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0);
    }

    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };

    match handle.ec.version_info() {
        Ok(build_info) => {
            *out_buffer = FrameworkByteBuffer::from_vec(build_info.into_bytes());
            FrameworkStatus::success()
        }
        Err(error) => {
            *out_buffer = FrameworkByteBuffer::default();
            status_from_error(error)
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn framework_ec_get_flash_versions(
    handle: *const FrameworkEcHandle,
    out_versions: *mut FrameworkEcFlashVersions,
) -> FrameworkStatus {
    if out_versions.is_null() {
        return FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0);
    }

    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };

    let Some((ro_version, rw_version, current_image)) = handle.ec.flash_version() else {
        return FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0);
    };

    *out_versions = FrameworkEcFlashVersions {
        current_image: current_image.into(),
        ro_version: copy_text_bytes(&ro_version),
        rw_version: copy_text_bytes(&rw_version),
    };

    FrameworkStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn framework_byte_buffer_free(buffer: FrameworkByteBuffer) {
    buffer.destroy();
}

#[no_mangle]
pub unsafe extern "C" fn framework_ec_get_power_snapshot(
    handle: *const FrameworkEcHandle,
    out_snapshot: *mut FrameworkPowerSnapshot,
) -> FrameworkStatus {
    if out_snapshot.is_null() {
        return FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0);
    }

    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };

    let Some(power_info) = power::power_info(&handle.ec) else {
        return FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0);
    };

    let mut snapshot = FrameworkPowerSnapshot {
        ac_present: u8::from(power_info.ac_present),
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

    *out_snapshot = snapshot;
    FrameworkStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn framework_ec_get_fan_capabilities(
    handle: *const FrameworkEcHandle,
    out_capabilities: *mut FrameworkFanCapabilities,
) -> FrameworkStatus {
    if out_capabilities.is_null() {
        return FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0);
    }

    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };

    let fan_control = match feature_enabled(&handle.ec, EcFeatureCode::PwmFan) {
        Ok(supported) => supported,
        Err(status) => return status,
    };
    let thermal = match feature_enabled(&handle.ec, EcFeatureCode::Thermal) {
        Ok(supported) => supported,
        Err(status) => return status,
    };

    let fan_count = power::thermal_snapshot(&handle.ec)
        .map(|snapshot| snapshot.fan_count)
        .unwrap_or(0);

    *out_capabilities = FrameworkFanCapabilities {
        fan_count,
        supports_fan_control: u8::from(fan_control),
        supports_thermal_reporting: u8::from(thermal),
        reserved: 0,
    };

    FrameworkStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn framework_ec_get_thermal_snapshot(
    handle: *const FrameworkEcHandle,
    out_snapshot: *mut FrameworkThermalSnapshot,
) -> FrameworkStatus {
    if out_snapshot.is_null() {
        return FrameworkStatus::with(FrameworkStatusCode::NullPointer, 0);
    }

    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };

    let Some(snapshot) = power::thermal_snapshot(&handle.ec) else {
        return FrameworkStatus::with(FrameworkStatusCode::DataUnavailable, 0);
    };

    let mut temperatures = [FrameworkTemperatureReading {
        state: FrameworkTemperatureState::NotPresent,
        celsius: 0,
        reserved: 0,
    }; THERMAL_SENSOR_COUNT];
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

    *out_snapshot = FrameworkThermalSnapshot {
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
    };

    FrameworkStatus::success()
}

#[no_mangle]
pub unsafe extern "C" fn framework_ec_set_fan_rpm(
    handle: *const FrameworkEcHandle,
    fan_index: i32,
    rpm: u32,
) -> FrameworkStatus {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };
    let fan_index = match parse_optional_fan_index(fan_index) {
        Ok(fan_index) => fan_index,
        Err(status) => return status,
    };

    match handle.ec.fan_set_rpm(fan_index, rpm) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn framework_ec_set_fan_duty(
    handle: *const FrameworkEcHandle,
    fan_index: i32,
    percent: u32,
) -> FrameworkStatus {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };
    let fan_index = match parse_optional_fan_index(fan_index) {
        Ok(fan_index) => fan_index,
        Err(status) => return status,
    };

    match handle.ec.fan_set_duty(fan_index, percent) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    }
}

#[no_mangle]
pub unsafe extern "C" fn framework_ec_restore_auto_fan_control(
    handle: *const FrameworkEcHandle,
    fan_index: i32,
) -> FrameworkStatus {
    let handle = match require_handle(handle) {
        Ok(handle) => handle,
        Err(status) => return status,
    };
    let fan_index = match parse_optional_fan_index_u8(fan_index) {
        Ok(fan_index) => fan_index,
        Err(status) => return status,
    };

    match handle.ec.autofanctrl(fan_index) {
        Ok(()) => FrameworkStatus::success(),
        Err(error) => status_from_error(error),
    }
}
