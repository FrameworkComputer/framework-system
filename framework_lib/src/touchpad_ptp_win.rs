use std::path::Path;

use hidapi::HidApi;
use windows::Win32::Devices::HumanInterfaceDevice::*;
use windows::Win32::Foundation::*;
use windows::Win32::Storage::FileSystem::*;

use crate::touchpad::PIX_VID;

const DIGITIZER_PAGE: u16 = 0x000D;
const USAGE_INPUT_MODE: u16 = 0x0052;

/// Get the current PTP input mode value.
/// Returns 0 for mouse mode, 3 for PTP mode.
pub fn get_ptp_mode() -> Option<u8> {
    let (handle, preparsed, report_len) = open_touchpad_device()?;

    let mut buf = vec![0u8; report_len];
    let report_id = get_input_mode_report_id(preparsed)?;
    buf[0] = report_id;

    let success = unsafe { HidD_GetFeature(handle, buf.as_mut_ptr() as _, buf.len() as u32) };
    if !success {
        error!("HidD_GetFeature failed");
        unsafe {
            HidD_FreePreparsedData(preparsed);
            let _ = CloseHandle(handle);
        }
        return None;
    }

    let mut value: u32 = 0;
    let status = unsafe {
        HidP_GetUsageValue(
            HidP_Feature,
            DIGITIZER_PAGE,
            None,
            USAGE_INPUT_MODE,
            &mut value,
            preparsed,
            &mut buf,
        )
    };

    unsafe {
        HidD_FreePreparsedData(preparsed);
        let _ = CloseHandle(handle);
    }

    if status != HIDP_STATUS_SUCCESS {
        error!("HidP_GetUsageValue failed: {:X}", status.0);
        return None;
    }

    Some(value as u8)
}

/// Set the PTP input mode value.
/// Use 0 for mouse mode, 3 for PTP mode.
pub fn set_ptp_mode(mode: u8) -> Option<()> {
    let (handle, preparsed, report_len) = open_touchpad_device()?;

    let report_id = get_input_mode_report_id(preparsed)?;
    let mut buf = vec![0u8; report_len];
    buf[0] = report_id;

    // Read current feature report
    let success = unsafe { HidD_GetFeature(handle, buf.as_mut_ptr() as _, buf.len() as u32) };
    if !success {
        error!("HidD_GetFeature failed");
        unsafe {
            HidD_FreePreparsedData(preparsed);
            let _ = CloseHandle(handle);
        }
        return None;
    }

    // Modify the Input Mode value
    let status = unsafe {
        HidP_SetUsageValue(
            HidP_Feature,
            DIGITIZER_PAGE,
            None,
            USAGE_INPUT_MODE,
            mode as u32,
            preparsed,
            &mut buf,
        )
    };

    if status != HIDP_STATUS_SUCCESS {
        error!("HidP_SetUsageValue failed: {:X}", status.0);
        unsafe {
            HidD_FreePreparsedData(preparsed);
            let _ = CloseHandle(handle);
        }
        return None;
    }

    // Write back
    let success = unsafe { HidD_SetFeature(handle, buf.as_ptr() as _, buf.len() as u32) };

    unsafe {
        HidD_FreePreparsedData(preparsed);
        let _ = CloseHandle(handle);
    }

    if !success {
        error!("HidD_SetFeature failed");
        return None;
    }

    Some(())
}

/// Open the touchpad device and return (handle, preparsed_data, feature_report_length).
fn open_touchpad_device() -> Option<(HANDLE, PHIDP_PREPARSED_DATA, usize)> {
    debug!("Looking for touchpad PTP device (Windows)");
    let api = match HidApi::new() {
        Ok(api) => api,
        Err(e) => {
            error!("Failed to open hidapi: {}", e);
            return None;
        }
    };

    for dev_info in api.device_list() {
        let vid = dev_info.vendor_id();
        let usage_page = dev_info.usage_page();

        if vid != PIX_VID || usage_page != DIGITIZER_PAGE {
            continue;
        }

        debug!(
            "  Found {:04X}:{:04X} (Usage Page {:04X})",
            vid,
            dev_info.product_id(),
            usage_page
        );

        let path_str = dev_info.path().to_str().ok()?;
        let handle = unsafe {
            CreateFileW(
                &windows::core::HSTRING::from(Path::new(path_str)),
                FILE_GENERIC_WRITE.0 | FILE_GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        };
        let handle = match handle {
            Ok(h) => h,
            Err(e) => {
                error!("Failed to open device: {:?}", e);
                continue;
            }
        };

        let mut preparsed = PHIDP_PREPARSED_DATA::default();
        let success = unsafe { HidD_GetPreparsedData(handle, &mut preparsed) };
        if !success {
            error!("HidD_GetPreparsedData failed");
            unsafe {
                let _ = CloseHandle(handle);
            }
            continue;
        }

        let mut caps = HIDP_CAPS::default();
        let status = unsafe { HidP_GetCaps(preparsed, &mut caps) };
        if status != HIDP_STATUS_SUCCESS {
            error!("HidP_GetCaps failed");
            unsafe {
                HidD_FreePreparsedData(preparsed);
                let _ = CloseHandle(handle);
            }
            continue;
        }

        let report_len = caps.FeatureReportByteLength as usize;
        if report_len == 0 {
            unsafe {
                HidD_FreePreparsedData(preparsed);
                let _ = CloseHandle(handle);
            }
            continue;
        }

        return Some((handle, preparsed, report_len));
    }

    error!("Could not find touchpad with PTP support");
    None
}

/// Find the report ID containing the Input Mode usage.
fn get_input_mode_report_id(preparsed: PHIDP_PREPARSED_DATA) -> Option<u8> {
    let mut num_caps: u16 = 0;
    let status =
        unsafe { HidP_GetValueCaps(HidP_Feature, std::ptr::null_mut(), &mut num_caps, preparsed) };
    if status != HIDP_STATUS_BUFFER_TOO_SMALL && status != HIDP_STATUS_SUCCESS {
        return None;
    }
    if num_caps == 0 {
        return None;
    }

    let mut caps_buf = vec![HIDP_VALUE_CAPS::default(); num_caps as usize];
    let status = unsafe {
        HidP_GetValueCaps(
            HidP_Feature,
            caps_buf.as_mut_ptr(),
            &mut num_caps,
            preparsed,
        )
    };
    if status != HIDP_STATUS_SUCCESS {
        return None;
    }

    for cap in &caps_buf[..num_caps as usize] {
        if cap.UsagePage == DIGITIZER_PAGE {
            let range = unsafe { cap.Anonymous.NotRange };
            if range.Usage == USAGE_INPUT_MODE {
                return Some(cap.ReportID);
            }
        }
    }

    None
}
