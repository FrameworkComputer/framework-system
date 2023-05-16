use hidapi::{HidApi, HidDevice};

use crate::ccgx::device::{decode_flash_row_size, FwMode};
use crate::ccgx::BaseVersion;
use crate::os_specific;

pub const CCG_USAGE_PAGE: u16 = 0xFFEE;

pub const FRAMEWORK_VID: u16 = 0x32AC;
pub const HDMI_CARD_PID: u16 = 0x0002;
pub const DP_CARD_PID: u16 = 0x0003;

const ROW_SIZE: usize = 128;
const FW1_START: u16 = 0x0030;
const FW2_START: u16 = 0x0200;
const FW1_METADATA: u16 = 0x03FF;
const FW2_METADATA: u16 = 0x03FE;

#[repr(packed)]
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

fn get_fw_info(device: &HidDevice) -> HidFirmwareInfo {
    device.set_blocking_mode(true).unwrap();

    // Same for both images
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
        .unwrap();

    // Returns Err but seems to work anyway. OH! Probably because it resets the device!!
    // TODO: I have a feeling the last five bytes are ignored. They're the same in all commands.
    //       Seems to work with all of them set to 0x00
    let _ = device.write(&[
        ReportIdCmd::E1Cmd as u8,
        CmdId::Cmd0x06 as u8,
        CmdParam::BridgeMode as u8,
        0x00,
        0xCC,
        0xCC,
        0xCC,
        0xCC,
    ]);

    // Get 0x40 bytes from 0xE0
    let mut buf = [0u8; 0x40];
    buf[0] = ReportIdCmd::E0Read as u8;
    device.get_feature_report(&mut buf).unwrap();

    decode_fw_info(&buf)
}

pub fn check_ccg_fw_version(device: &HidDevice) {
    let info = get_fw_info(device);
    print_fw_info(&info);
}

//  0 ..  2  = HID header
//  2 ..  4  = signature (CY)
//  4        = Operating Mode
//  5        = Bootloader (security, no-flashing, priority, row_size)
//  6        = Boot mode reason, jump-bootloader, reserved, fw1 invalid, fw2 invalid
//  7        = ??
//  8 .. 12  = Silicon ID
// 12 .. 16  = bootloader Version
// 15 .. 20  = ??
// 20 .. 24  = Image 1 Version
// 24 .. 26  = Image 1 ??
// 26 .. 28  = Image 1 ??
// 28 .. 32  = Image 2 Version
// 32 .. 34  = Image 2 ??
// 34 .. 36  = Image 2 ??
// 36 .. 40  = Image 1 Start Address
// 40 .. 44  = Image 2 Start Address
// 44 .. 52  = ?? [c9 d7 3e 02 23 19 0b 00]

fn decode_fw_info(buf: &[u8]) -> HidFirmwareInfo {
    let info_len = std::mem::size_of::<HidFirmwareInfo>();
    let info: HidFirmwareInfo = unsafe { std::ptr::read(buf[..info_len].as_ptr() as *const _) };

    // TODO: Return Option?
    assert_eq!(info.report_id, ReportIdCmd::E0Read as u8);
    assert_eq!(info.signature, [b'C', b'Y']);

    info
}

fn print_fw_info(info: &HidFirmwareInfo) {
    assert_eq!(info.report_id, ReportIdCmd::E0Read as u8);

    debug!("  Signature:            {:X?}", info.signature);
    // Something's totally off if the signature is invalid
    assert_eq!(info.signature, [b'C', b'Y']);

    debug!(
        "  Operating Mode:       {:?} ({})",
        FwMode::try_from(info.operating_mode).unwrap(),
        info.operating_mode
    );
    debug!("  Bootloader Info");
    debug!(
        "    Security Support:   {:?}",
        info.bootloader_info & 0b001 != 0
    );
    debug!(
        "    Flashing Support:   {:?}",
        info.bootloader_info & 0b010 == 0
    );
    debug!(
        "    App Priority:       {:?}",
        info.bootloader_info & 0b100 != 0
    );
    debug!(
        "    Flash Row Size:     {:?}",
        decode_flash_row_size(info.bootloader_info)
    );
    debug!("  Boot Mode Reason");
    debug!(
        "    Jump to Bootloader: {:?}",
        info.bootmode_reason & 0b000001 != 0
    );
    let image_1_valid = info.bootmode_reason & 0b000100 == 0;
    let image_2_valid = info.bootmode_reason & 0b001000 == 0;
    debug!("    FW 1 valid:         {:?}", image_1_valid);
    debug!("    FW 2 valid:         {:?}", image_2_valid);
    debug!(
        "    App Priority:       {:?}",
        info.bootmode_reason & 0b110000
    );
    debug!("    UID:                {:X?}", info.device_uid);
    debug!("  Silicon ID:      {:X?}", info.silicon_id);
    let bl_ver = BaseVersion::from(info.bl_version.as_slice());
    let base_version_1 = BaseVersion::from(info.image_1_ver.as_slice());
    let base_version_2 = BaseVersion::from(info.image_2_ver.as_slice());
    debug!(
        "  BL Version:      {} Build {}",
        bl_ver, bl_ver.build_number
    );
    debug!(
        "  Image 1 start:   0x{:08X}",
        u32::from_le_bytes(info.image_1_row)
    );
    debug!(
        "  Image 2 start:   0x{:08X}",
        u32::from_le_bytes(info.image_2_row)
    );

    println!(
        "  FW Image 1 Version:   {:03} ({}){}",
        base_version_1.build_number,
        base_version_1,
        if image_1_valid { "" } else { " - INVALID!" }
    );
    println!(
        "  FW Image 2 Version:   {:03} ({}){}",
        base_version_2.build_number,
        base_version_2,
        if image_2_valid { "" } else { " - INVALID!" }
    );
    println!(
        "  Currently running:    {:?} ({})",
        FwMode::try_from(info.operating_mode).unwrap(),
        info.operating_mode
    );
}

pub fn device_name(vid: u16, pid: u16) -> Option<&'static str> {
    match (vid, pid) {
        (FRAMEWORK_VID, HDMI_CARD_PID) => Some("HDMI Expansion Card"),
        (FRAMEWORK_VID, DP_CARD_PID) => Some("DisplayPort Expansion Card"),
        _ => None,
    }
}

pub fn find_device(api: &HidApi) -> Option<HidDevice> {
    for dev_info in api.device_list() {
        let vid = dev_info.vendor_id();
        let pid = dev_info.product_id();
        let usage_page = dev_info.usage_page();
        if vid == FRAMEWORK_VID && [DP_CARD_PID, HDMI_CARD_PID].contains(&pid) && usage_page == CCG_USAGE_PAGE {
            return Some(dev_info.open_device(api).unwrap());
        }
    }
    None
}

pub fn flash_firmware(fw_binary: &[u8]) {
    // Make sure the firmware is composed of rows and has two images
    // The assumption is that both images are of the same size
    assert_eq!(fw_binary.len() % 2 * ROW_SIZE, 0);
    let fw_size = fw_binary.len() / 2;
    let fw1_binary = &fw_binary[..fw_size];
    let fw2_binary = &fw_binary[fw_size..];

    // First update the one that's not currently running.
    // After updating the first image, the device restarts and boots into the other one.
    // Then we need to re-enumerate the USB devices because it'll change device id
    let mut api = HidApi::new().unwrap();
    let device = if let Some(device) = find_device(&api) {
        device
    } else {
        error!("No compatible Expansion Card connected");
        return;
    };
    let info = get_fw_info(&device);
    println!("Before Updating");
    print_fw_info(&info);

    println!("Updating...");
    match info.operating_mode {
        // I think in bootloader mode we can update either one first. Never tested
        0 | 2 => {
            println!("  Updating Firmware Image 1");
            flash_firmware_image(&device, fw1_binary, FW1_START, FW1_METADATA, 1);

            println!("  Waiting 4s for device to restart");
            os_specific::sleep(4_000_000);
            api.refresh_devices().unwrap();
            let device = find_device(&api).unwrap();

            println!("  Updating Firmware Image 2");
            flash_firmware_image(&device, fw2_binary, FW2_START, FW2_METADATA, 2);
        }
        1 => {
            println!("  Updating Firmware Image 2");
            flash_firmware_image(&device, fw2_binary, FW2_START, FW2_METADATA, 2);

            println!("  Waiting 4s for device to restart");
            os_specific::sleep(4_000_000);
            api.refresh_devices().unwrap();
            let device = find_device(&api).unwrap();

            println!("  Updating Firmware Image 1");
            flash_firmware_image(&device, fw1_binary, FW1_START, FW1_METADATA, 1);
        }
        _ => unreachable!(),
    }

    println!("  Firmware Update done.");
    println!("  Waiting 4s for device to restart");
    os_specific::sleep(4_000_000);

    println!("After Updating");
    api.refresh_devices().unwrap();
    let device = find_device(&api).unwrap();
    let info = get_fw_info(&device);
    print_fw_info(&info);
}

fn flash_firmware_image(
    device: &HidDevice,
    fw_binary: &[u8],
    start_row: u16,
    metadata_row: u16,
    no: u8,
) {
    // Should be roughly 460 plus/minus 2
    debug!("Chunks: {:?}", fw_binary.len() / ROW_SIZE);
    assert_eq!(fw_binary.len() % ROW_SIZE, 0);

    device.set_blocking_mode(true).unwrap();

    // Same for both images
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
        .unwrap();

    // Returns Err but seems to work anyway. OH! Probably because it resets the device!!
    // TODO: I have a feeling the last five bytes are ignored. They're the same in all commands.
    //       Seems to work with all of them set to 0x00
    let _ = device.write(&[
        ReportIdCmd::E1Cmd as u8,
        CmdId::Cmd0x06 as u8,
        CmdParam::BridgeMode as u8,
        0x00,
        0xCC,
        0xCC,
        0xCC,
        0xCC,
    ]);

    // Probably enter flashing mode?
    let _ = device
        .write(&[
            ReportIdCmd::E1Cmd as u8,
            CmdId::CmdFlash as u8,
            CmdParam::Enable as u8,
            0x00,
            0xCC,
            0xCC,
            0xCC,
            0xCC,
        ])
        .unwrap();

    // TODO: Probably not necessary?
    let mut buf = [0u8; 0x40];
    buf[0] = ReportIdCmd::E0Read as u8;
    device.get_feature_report(&mut buf).unwrap();

    // Why another time enter flashing mode?
    let _ = device
        .write(&[
            ReportIdCmd::E1Cmd as u8,
            CmdId::CmdFlash as u8,
            CmdParam::Enable as u8,
            0x00,
            0xCC,
            0xCC,
            0xCC,
            0xCC,
        ])
        .unwrap();

    let rows = fw_binary.chunks(ROW_SIZE);
    let last_row = (rows.len() - 1) as u16;
    for (row_no, row) in rows.enumerate() {
        assert_eq!(row.len(), ROW_SIZE);
        let row_no = row_no as u16;
        if row_no == last_row {
            write_row(device, metadata_row, row);
        } else {
            write_row(device, start_row + row_no, row);
        }
    }

    // Not quite sure what this is. But on the first update it has
    // 0x01 and on the second it has 0x02. So I think this switches the boot order?
    let _ = device
        .write(&[
            ReportIdCmd::E1Cmd as u8,
            0x04,
            no,
            0x00,
            0xCC,
            0xCC,
            0xCC,
            0xCC,
        ])
        .unwrap();

    // Seems to reset the device, since the USB device number changes
    let _ = device
        .write(&[
            ReportIdCmd::E1Cmd as u8,
            CmdId::CmdJump as u8,
            CmdParam::Reset as u8,
            0x00,
            0xCC,
            0xCC,
            0xCC,
            0xCC,
        ])
        .unwrap();
}

fn write_row(device: &HidDevice, row_no: u16, row: &[u8]) {
    let row_no_bytes = row_no.to_le_bytes();
    debug!("Writing row {:04X}. Data: {:X?}", row_no, row);

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
    device.write(&buffer).unwrap();
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
