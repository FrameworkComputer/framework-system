use core::time::Duration;

use crate::util;
use rusb::{DeviceHandle, Direction, GlobalContext, Recipient, RequestType};

pub const FRAMEWORK_VID: u16 = 0x32AC;
pub const AUDIO_CARD_PID: u16 = 0x0010;

const CAPE_DATA_LEN: usize = 13;
const CAPE_MODULE_ID: u32 = 0xB32D2300;
const CAPE_REPORT_ID: u16 = 0x0001;

#[repr(u16)]
enum CapeCommand {
    GetVersion = 0x0103,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct CapeMessage {
    _len: i16,
    command_id: u16,
    //request_reply: last bit of command_id
    _module_id: u32,
    data: [u32; CAPE_DATA_LEN],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct HidCapeMessage {
    _report_id: u16,
    msg: CapeMessage,
}

/// Get and print the firmware version of connected audio cards
///
/// Unfortunately this needs to open the USB device and claim the interface,
/// as well as detach currently connected kernel drivers.
/// This is most likely the case because it's using the Consumer Control usage page.
pub fn check_synaptics_fw_version() {
    let mut audio_cards = 0;
    for dev in rusb::devices().unwrap().iter() {
        let dev_descriptor = dev.device_descriptor().unwrap();
        if dev_descriptor.vendor_id() != FRAMEWORK_VID
            || dev_descriptor.product_id() != AUDIO_CARD_PID
        {
            continue;
        }
        let handle = dev.open().unwrap();

        let interface_number = if let Some(num) = find_hid_interface(&handle) {
            num
        } else {
            error!("Couldn't open Framework Audio Card - No HID Interface");
            continue;
        };
        audio_cards += 1;

        // On Linux it's claimed by a kernel driver, so we need to detach that to make it usable for us.
        // On Windows this panics with "NotSupported" and it seems not required.
        #[cfg(target_os = "linux")]
        handle.set_auto_detach_kernel_driver(true).unwrap();

        handle.claim_interface(interface_number).unwrap();
        let timeout = std::time::Duration::from_millis(100);

        let request = HidCapeMessage {
            _report_id: CAPE_REPORT_ID,
            msg: CapeMessage {
                _len: (CAPE_DATA_LEN as i16).to_le(),
                command_id: (CapeCommand::GetVersion as u16).to_le(),
                _module_id: CAPE_MODULE_ID,
                data: [0; CAPE_DATA_LEN],
            },
        };
        let mut response = request;

        // 0x81 means a valid response is ready
        while response.msg.command_id != 0x8103 {
            let index = interface_number as u16;
            set_hid_report(
                &handle,
                1,
                index,
                unsafe { util::any_as_u8_slice(&request) },
                timeout,
            )
            .unwrap();
            let res = get_hid_report(
                &handle,
                1,
                index,
                unsafe { util::any_as_mut_u8_slice(&mut response) },
                timeout,
            );
            assert_eq!(res, Ok(core::mem::size_of::<HidCapeMessage>()));
        }

        let version = &{ response.msg.data }[0..4];
        println!("Audio Expansion Card");
        println!(
            "  Firmware Version: {}.{}.{}.{}",
            version[0], version[1], version[2], version[3]
        );

        let dev_descriptor = dev.device_descriptor().unwrap();
        let i_serial = dev_descriptor
            .serial_number_string_index()
            .and_then(|x| handle.read_string_descriptor_ascii(x).ok());
        let i_product = dev_descriptor
            .product_string_index()
            .and_then(|x| handle.read_string_descriptor_ascii(x).ok());
        println!("  bcdDevice:        {}", dev_descriptor.device_version());
        println!("  iSerial:          {:?}", i_serial.unwrap_or_default());
        println!("  iProduct          {:?}", i_product.unwrap_or_default());
    }

    if audio_cards == 0 {
        error!("No Framework Audio Cards detected");
    }
}

#[repr(u8)]
enum HidRequestType {
    GetReport = 0x01,
    SetReport = 0x09,
}

#[repr(u8)]
enum HidReportType {
    InputReport = 0x01,
    OutputReport = 0x02,
}

fn set_hid_report(
    handle: &DeviceHandle<GlobalContext>,
    report_id: u8,
    index: u16,
    buf: &[u8],
    timeout: Duration,
) -> rusb::Result<usize> {
    let request_type = rusb::request_type(Direction::Out, RequestType::Class, Recipient::Interface);
    handle.write_control(
        request_type,
        HidRequestType::SetReport as u8,
        u16::from_le_bytes([report_id, HidReportType::OutputReport as u8]),
        index,
        buf,
        timeout,
    )
}

fn get_hid_report(
    handle: &DeviceHandle<GlobalContext>,
    report_id: u8,
    index: u16,
    buf: &mut [u8],
    timeout: Duration,
) -> rusb::Result<usize> {
    let request_type = rusb::request_type(Direction::In, RequestType::Class, Recipient::Interface);
    handle.read_control(
        request_type,
        HidRequestType::GetReport as u8,
        u16::from_le_bytes([report_id, HidReportType::InputReport as u8]),
        index,
        buf,
        timeout,
    )
}

fn find_hid_interface(handle: &DeviceHandle<GlobalContext>) -> Option<u8> {
    let dev = handle.device();
    let config = dev.active_config_descriptor().unwrap();
    for interface in config.interfaces() {
        for descriptor in interface.descriptors() {
            if descriptor.class_code() == 0x03 {
                return Some(interface.number());
            }
        }
    }
    None
}
