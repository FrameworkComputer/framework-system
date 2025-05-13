use hidapi::{DeviceInfo, HidApi, HidDevice, HidError};

use crate::ccgx;
use crate::ccgx::device::{decode_flash_row_size, FwMode};
use crate::ccgx::{BaseVersion, SiliconId};
use crate::os_specific;
use crate::util;

pub const CCG_USAGE_PAGE: u16 = 0xFFEE;

pub const FRAMEWORK_VID: u16 = 0x32AC;
pub const HDMI_CARD_PID: u16 = 0x0002;
pub const DP_CARD_PID: u16 = 0x0003;
pub const ALL_CARD_PIDS: [u16; 2] = [DP_CARD_PID, HDMI_CARD_PID];

/// It takes as little as 3s but sometimes more than 5s for the HDMI/DP cards
/// to restart and enumerate in the OS
/// Check every 0.5s for up to 10s
pub const RESTART_TIMEOUT: u64 = 10_000_000;
pub const RESTART_PERIOD: u64 = 500_000;

const ROW_SIZE: usize = 128;
const FW1_START: usize = 0x0030;
const FW2_START: usize = 0x0200;
const FW1_METADATA: usize = 0x03FF;
const FW2_METADATA: usize = 0x03FE;

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
struct HidFirmwareInfo {
    report_id: u8,
    _reserved_1: u8,
    signature: [u8; 2],
    operating_mode: u8,
    bootloader_info: u8,
    bootmode_reason: u8,
    _reserved_2: u8,
    silicon_id: [u8; 4],
    bl_version: [u8; 8],
    image_1_ver: [u8; 8],
    image_2_ver: [u8; 8],
    image_1_row: [u8; 4],
    image_2_row: [u8; 4],
    device_uid: [u8; 6],
    _reserved_3: [u8; 10],
}

#[repr(u8)]
enum CmdId {
    CmdJump = 0x01,
    /// Seems to enter flashing mode
    CmdFlash = 0x02,
    /// Not quite sure what it does
    Cmd0x04 = 0x04,
    /// Some sort of mode switch
    Cmd0x06 = 0x06,
}

#[repr(u8)]
enum CmdParam {
    /// A - Untested
    _JumpToAlternateImage = 0x41,
    /// B
    BridgeMode = 0x42,
    /// F
    FlashWrite = 0x46,
    /// J - Untested
    _JumpToBootloader = 0x4A,
    /// P - Enable flashing mode
    Enable = 0x50,
    /// R
    Reset = 0x52,
}

#[repr(u8)]
enum ReportIdCmd {
    /// 224. Maybe read register?
    E0Read = 0xE0,
    /// 225. 7 bytes
    E1Cmd = 0xE1,
    /// 226 - Write a row of firmware. 131 bytes payload
    E2WriteRow = 0xE2,
    /// 227 - Haven't seen it used. Maybe it's read?
    _E3 = 0xE3,
    /// 228 - Maybe write register?
    E4 = 0xE4,
}

fn flashing_mode(device: &HidDevice) {
    // Probably enter flashing mode?
    info!("Enter flashing mode");
    let _ = send_command(device, CmdId::CmdFlash, CmdParam::Enable as u8)
        .expect("Failed to enter flashing mode");
}

fn magic_unlock(device: &HidDevice) {
    device.set_blocking_mode(true).unwrap();

    // Same for both images
    info!("Magic unlock");
    device
        .send_feature_report(&[
            ReportIdCmd::E4 as u8,
            0x42,
            0x43,
            0x59,
            0x00,
            0x00,
            0x00,
            0x0B,
        ])
        .expect("Failed to unlock device");

    // Returns Err but seems to work anyway. OH! Probably because it resets the device!!
    // TODO: I have a feeling the last five bytes are ignored. They're the same in all commands.
    //       Seems to work with all of them set to 0x00
    info!("Bridge Mode");
    let _ = send_command(device, CmdId::Cmd0x06, CmdParam::BridgeMode as u8);
}

fn get_fw_info(device: &HidDevice) -> HidFirmwareInfo {
    // Get 0x40 bytes from 0xE0
    let mut buf = [0u8; 0x40];
    buf[0] = ReportIdCmd::E0Read as u8;
    info!("Get Report E0");
    device
        .get_feature_report(&mut buf)
        .expect("Failed to get device FW info");

    flashing_mode(device);

    decode_fw_info(&buf)
}

pub fn check_ccg_fw_version(device: &HidDevice, verbose: bool) {
    magic_unlock(device);
    let info = get_fw_info(device);
    print_fw_info(&info, verbose);
}

fn decode_fw_info(buf: &[u8]) -> HidFirmwareInfo {
    let info_len = std::mem::size_of::<HidFirmwareInfo>();
    let info: HidFirmwareInfo = unsafe { std::ptr::read(buf[..info_len].as_ptr() as *const _) };

    // TODO: Return Option?
    assert_eq!(info.report_id, ReportIdCmd::E0Read as u8);
    if info.signature != [b'C', b'Y'] {
        println!("{:X?}", info);
    }
    assert_eq!(info.signature, [b'C', b'Y']);

    info
}

fn print_fw_info(info: &HidFirmwareInfo, verbose: bool) {
    assert_eq!(info.report_id, ReportIdCmd::E0Read as u8);

    info!("  Signature:            {:X?}", info.signature);
    // Something's totally off if the signature is invalid
    if info.signature != [b'C', b'Y'] {
        error!("Firmware Signature is invalid.");
        return;
    }

    info!("  Bootloader Info");
    info!(
        "    Security Support:   {:?}",
        info.bootloader_info & 0b001 != 0
    );
    info!(
        "    Flashing Support:   {:?}",
        info.bootloader_info & 0b010 == 0
    );
    // App Priority means you can configure whether the main or backup firmware
    // has priority. This can either be configured in the flash image or by
    // sending a command. But the CCG3 SDK lets you disable support for this at
    // compile-time. If disabled, both images have the same priority.
    let app_priority_support = info.bootloader_info & 0b100 != 0;
    info!("    App Priority:       {:?}", app_priority_support);
    info!(
        "    Flash Row Size:     {:?} B",
        decode_flash_row_size(info.bootloader_info)
    );
    info!("  Boot Mode Reason");
    info!(
        "    Jump to Bootloader: {:?}",
        info.bootmode_reason & 0b000001 != 0
    );
    let image_1_valid = info.bootmode_reason & 0b000100 == 0;
    let image_2_valid = info.bootmode_reason & 0b001000 == 0;
    info!("    FW 1 valid:         {:?}", image_1_valid);
    info!("    FW 2 valid:         {:?}", image_2_valid);
    if app_priority_support {
        info!(
            "    App Priority:       {:?}",
            info.bootmode_reason & 0b110000
        );
    }
    info!("    UID:                {:X?}", info.device_uid);
    info!("  Silicon ID:      {:X?}", info.silicon_id);
    let bl_ver = BaseVersion::from(info.bl_version.as_slice());
    let base_version_1 = BaseVersion::from(info.image_1_ver.as_slice());
    let base_version_2 = BaseVersion::from(info.image_2_ver.as_slice());
    info!("  BL Version:      {} ", bl_ver,);
    info!(
        "  Image 1 start:   0x{:08X}",
        u32::from_le_bytes(info.image_1_row)
    );
    info!(
        "  Image 2 start:   0x{:08X}",
        u32::from_le_bytes(info.image_2_row)
    );

    let operating_mode = FwMode::try_from(info.operating_mode).unwrap();
    let (active_ver, active_valid, inactive_ver, inactive_valid) = match operating_mode {
        FwMode::MainFw | FwMode::BootLoader => {
            (base_version_2, image_2_valid, base_version_1, image_1_valid)
        }
        FwMode::BackupFw => (base_version_1, image_1_valid, base_version_2, image_2_valid),
    };

    if verbose || active_ver != inactive_ver {
        println!(
            "  Active Firmware:      {:03} ({}){}",
            active_ver.build_number,
            active_ver,
            if active_valid { "" } else { " - INVALID!" }
        );
        println!(
            "  Inactive Firmware:    {:03} ({}){}",
            inactive_ver.build_number,
            inactive_ver,
            if inactive_valid { "" } else { " - INVALID!" }
        );
        println!(
            "  Operating Mode:       {:?} (#{})",
            FwMode::try_from(info.operating_mode).unwrap(),
            info.operating_mode
        );
    } else {
        println!(
            "  Active Firmware:  {:03} ({}, {:?}){}",
            active_ver.build_number,
            active_ver,
            FwMode::try_from(info.operating_mode).unwrap(),
            if active_valid { "" } else { " - INVALID!" }
        );
    }
}

/// Turn CCG3 Expansion Card VID/PID into their name
pub fn device_name(vid: u16, pid: u16) -> Option<&'static str> {
    match (vid, pid) {
        (FRAMEWORK_VID, HDMI_CARD_PID) => Some("HDMI Expansion Card"),
        (FRAMEWORK_VID, DP_CARD_PID) => Some("DisplayPort Expansion Card"),
        _ => None,
    }
}

/// Find HDMI/DP Expansion cards, optionally filter by product ID or serial number
pub fn find_devices(api: &HidApi, filter_devs: &[u16], sn: Option<&str>) -> Vec<DeviceInfo> {
    api.device_list()
        .filter_map(|dev_info| {
            let vid = dev_info.vendor_id();
            let pid = dev_info.product_id();
            let usage_page = dev_info.usage_page();

            debug!("Found {:X}:{:X} Usage Page: {}", vid, pid, usage_page);
            #[cfg(not(target_os = "freebsd"))]
            let usage_page_filter = usage_page == CCG_USAGE_PAGE;
            // On FreeBSD it seems we don't get different usage pages
            // There's just one entry overall
            #[cfg(target_os = "freebsd")]
            let usage_page_filter = true;

            if vid == FRAMEWORK_VID
                && filter_devs.contains(&pid)
                && usage_page_filter
                && (sn.is_none() || sn == dev_info.serial_number())
            {
                Some(dev_info.clone())
            } else {
                None
            }
        })
        .collect()
}

pub fn flash_firmware(fw_binary: &[u8]) {
    let versions = if let Some(versions) = ccgx::binary::read_versions(fw_binary, SiliconId::Ccg3) {
        versions
    } else {
        println!("Incompatible firmware. Need CCG3 firmware.");
        return;
    };

    // Not sure if there's a better way to check whether the firmware is for DP or HDMI card
    let dp_string = b"F\0r\0a\0m\0e\0w\0o\0r\0k\x006\x03D\0i\0s\0p\0l\0a\0y\0P\0o\0r\0t\0 \0E\0x\0p\0a\0n\0s\0i\0o\0n\0 \0C\0a\0r\0d\0";
    let hdmi_string = b"F\0r\0a\0m\0e\0w\0o\0r\0k\0(\x03H\0D\0M\0I\0 \0E\0x\0p\0a\0n\0s\0i\0o\0n\0 \0C\0a\0r\0d\0";
    let filter_devs = if util::find_sequence(fw_binary, hdmi_string).is_some() {
        [HDMI_CARD_PID]
    } else if util::find_sequence(fw_binary, dp_string).is_some() {
        [DP_CARD_PID]
    } else {
        println!("Incompatible firmware. Need DP/HDMI Expansion Card Firmware.");
        return;
    };

    let fw1_rows = versions.backup_fw.size / versions.backup_fw.row_size;
    let fw2_rows = versions.main_fw.size / versions.main_fw.row_size;

    println!("File Firmware:");
    println!("  {}", device_name(FRAMEWORK_VID, filter_devs[0]).unwrap());
    println!("  {}", versions.main_fw.base_version);

    // First update the one that's not currently running.
    // After updating the first image, the device restarts and boots into the other one.
    // Then we need to re-enumerate the USB devices because it'll change device id
    let mut api = HidApi::new().unwrap();
    let devices = find_devices(&api, &filter_devs, None);
    if devices.is_empty() {
        println!("No compatible Expansion Card connected");
        return;
    };
    for dev_info in devices {
        // Unfortunately the HID API doesn't allow us to introspect the USB
        // topology because it abstracts USB, Bluetooth and other HID devices.
        // The libusb API does allow that but it's lower level and requires
        // root privileges on Linux.
        // So we can't figure out which port the card is connected to.
        // Would be nice to show that instead of the serial number.
        // We want to show that so the user knows that multiple *different*
        // cards are being updated.
        let sn = dev_info
            .serial_number()
            .expect("Device has no serial number");
        let dev_name = device_name(dev_info.vendor_id(), dev_info.product_id()).unwrap();
        println!();
        println!("Updating {} with SN: {:?}", dev_name, sn);

        let device = dev_info.open_device(&api).unwrap();
        magic_unlock(&device);
        let info = get_fw_info(&device);
        println!("Before Updating");
        print_fw_info(&info, true);

        println!("Updating...");
        match info.operating_mode {
            // I think in bootloader mode we can update either one first. Never tested
            0 | 2 => {
                println!("  Updating Firmware Image 1");
                flash_firmware_image(&device, fw_binary, FW1_START, FW1_METADATA, fw1_rows, 1);

                // We don't actually need to update both firmware images.
                // It'll stay on the one we updated. So it's totally fine to
                // keep the other one on the older version.
                //let (device, _) =
                //    wait_to_reappear(&mut api, &filter_devs, sn).expect("Device did not reappear");

                //println!("  Updating Firmware Image 2");
                //flash_firmware_image(&device, fw_binary, FW2_START, FW2_METADATA, fw2_rows, 2);
            }
            1 => {
                println!("  Updating Firmware Image 2");
                flash_firmware_image(&device, fw_binary, FW2_START, FW2_METADATA, fw2_rows, 2);

                // See above
                //let (device, _) =
                //    wait_to_reappear(&mut api, &filter_devs, sn).expect("Device did not reappear");

                //println!("  Updating Firmware Image 1");
                //flash_firmware_image(&device, fw_binary, FW1_START, FW1_METADATA, fw1_rows, 1);
            }
            _ => unreachable!(),
        }

        println!("  Firmware Update done.");
        let (_, info) =
            wait_to_reappear(&mut api, &filter_devs, sn).expect("Device did not reappear");

        println!("After Updating");
        print_fw_info(&info, true);
    }
}

fn flash_firmware_image(
    device: &HidDevice,
    fw_binary: &[u8],
    start_row: usize,
    metadata_row: usize,
    rows: usize,
    no: u8,
) {
    let fw_slice = &fw_binary[start_row * ROW_SIZE..(start_row + rows) * ROW_SIZE];
    let metadata_slice = &fw_binary[metadata_row * ROW_SIZE..(metadata_row + 1) * ROW_SIZE];
    // Should be roughly 460 plus/minus 2
    debug!("Chunks: {:?}", (fw_slice.len() / ROW_SIZE) + 1);

    let _info = get_fw_info(device);

    let rows = fw_slice.chunks(ROW_SIZE);
    for (row_no, row) in rows.enumerate() {
        assert_eq!(row.len(), ROW_SIZE);
        if row_no == 0 {
            info!(
                "Writing first firmware row@{:X?}: {:X?}",
                start_row + row_no,
                row
            );
        }
        write_row(device, (start_row + row_no) as u16, row).unwrap_or_else(|err| {
            panic!(
                "Failed to write firmware row #{} (@{:X}): {:?}",
                row_no,
                start_row + row_no,
                err
            )
        });
    }
    info!(
        "Writing metadata       row@{:X?}: {:X?}",
        metadata_row, metadata_slice
    );
    write_row(device, metadata_row as u16, metadata_slice)
        .expect("Failed to write firmware metadata");

    // Not quite sure what this is. But on the first update it has
    // 0x01 and on the second it has 0x02. So I think this switches the boot order?
    info!("Bootswitch");
    let _ = send_command(device, CmdId::Cmd0x04, no).unwrap();

    // Seems to reset the device, since the USB device number changes
    info!("Reset");
    let _ = send_command(device, CmdId::CmdJump, CmdParam::Reset as u8).unwrap();
}

fn send_command(device: &HidDevice, cmd_id: CmdId, cmd_param: u8) -> Result<usize, HidError> {
    device.write(&[
        ReportIdCmd::E1Cmd as u8,
        cmd_id as u8,
        cmd_param,
        0x00,
        0xCC,
        0xCC,
        0xCC,
        0xCC,
    ])
}

fn write_row(device: &HidDevice, row_no: u16, row: &[u8]) -> Result<usize, HidError> {
    let row_no_bytes = row_no.to_le_bytes();
    trace!("Writing row {:04X}. Data: {:X?}", row_no, row);

    // First image start 0x1800 (row 0x30)
    // row from 0x0030 to 0x01fb
    // 0x30*128 (row size) = 0x1800, which is Image 1 start = 00001800
    // 0x01fb-0x30+1 = 460
    // OH! This must be the metadata
    // And another one at 03ff, which adds up to 461 rows
    //
    // Second Image start 0x10000 (row 0x200)
    // Last one if 0x03cb
    // And another one at 03fe

    // I think 0x46 might be CY_PD_FLASH_READ_WRITE_CMD_SIG
    let mut buffer = [0; 132];
    buffer[0] = ReportIdCmd::E2WriteRow as u8;
    buffer[1] = CmdParam::FlashWrite as u8;
    buffer[2] = row_no_bytes[0];
    buffer[3] = row_no_bytes[1];
    buffer[4..].copy_from_slice(row);
    device.write(&buffer)
}

/// Wait for the specific card to reappear
/// Waiting a maximum timeout, if it hasn't appeared by then, return None
fn wait_to_reappear(
    api: &mut HidApi,
    filter_devs: &[u16],
    sn: &str,
) -> Option<(HidDevice, HidFirmwareInfo)> {
    println!("  Waiting for Expansion Card to restart");
    let retries = RESTART_TIMEOUT / RESTART_PERIOD;

    for i in (0..retries).rev() {
        os_specific::sleep(RESTART_PERIOD);
        api.refresh_devices().unwrap();
        let new_devices = find_devices(api, filter_devs, Some(sn));
        if new_devices.is_empty() {
            debug!("No devices found, retrying #{}/{}", retries - i, retries);
            continue;
        }
        let dev_info = &new_devices[0];
        if let Ok(device) = dev_info.open_device(api) {
            magic_unlock(&device);
            let info = get_fw_info(&device);
            return Some((device, info));
        }
    }
    None
}

// HID Report on device before updating Firmware 2
//     Usage Page (Vendor)
//         Header
//         Usage Page: Vendor (0xffee)
//     Usage (Vendor)
//         Header
//         Usage: Vendor (0x01)
//     Collection (Application)
//         Header
//         Collection type: Application (0x01)
//         Report ID (0xe0)
//             Usage (Vendor)
//             Logical Minimum (0)
//             Logical Maximum (255)
//             Report Size (8)
//             Report Count (63)
//             Feature (Data,Var,Abs)
//         Report ID (0xe1)
//             Usage (Vendor)
//             Logical Minimum (0)
//             Logical Maximum (255)
//             Report Size (8)
//             Report Count (7)
//             Output (Data,Var,Abs)
//         Report ID (0xe2)
//             Usage (Vendor)
//             Logical Minimum (0)
//             Logical Maximum (255)
//             Report Size (8)
//             Report Count (131)
//             Output (Data,Var,Abs)
//         Report ID (0xe3)
//             Usage (Vendor)
//             Logical Minimum (0)
//             Logical Maximum (255)
//             Report Size (8)
//             Report Count (131)
//             Feature (Data,Var,Abs)
//         Report ID (0xe4)
//             Usage (Vendor)
//             Logical Minimum (0)
//             Logical Maximum (255)
//             Report Size (8)
//             Report Count (7)
//             Feature (Data,Var,Abs)
//         End Collection
