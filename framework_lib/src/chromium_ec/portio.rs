use crate::chromium_ec::{EcError, EcResponseStatus, EcResult};
use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::TryInto;
#[cfg(any(feature = "linux_pio", feature = "freebsd_pio", feature = "raw_pio"))]
use hwio::{Io, Pio};
#[cfg(all(feature = "linux_pio", target_os = "linux"))]
use libc::ioperm;
use log::Level;
#[cfg(feature = "linux_pio")]
use nix::unistd::Uid;
use num::FromPrimitive;
#[cfg(feature = "linux_pio")]
use std::sync::{Arc, Mutex};

use crate::chromium_ec::{has_mec, portio_mec};
use crate::os_specific;
use crate::util;

/*
 * Value written to legacy command port / prefix byte to indicate protocol
 * 3+ structs are being used.  Usage is bus-dependent.
 */
const EC_COMMAND_PROTOCOL_3: u8 = 0xda;

// LPC command status byte masks
/// EC has written data but host hasn't consumed it yet
const _EC_LPC_STATUS_TO_HOST: u8 = 0x01;
/// Host has written data/command but EC hasn't consumed it yet
const EC_LPC_STATUS_FROM_HOST: u8 = 0x02;
/// EC is still processing a command
const EC_LPC_STATUS_PROCESSING: u8 = 0x04;
/// Previous command wasn't data but command
const _EC_LPC_STATUS_LAST_CMD: u8 = 0x08;
/// EC is in burst mode
const _EC_LPC_STATUS_BURST_MODE: u8 = 0x10;
/// SCI event is pending (requesting SCI query)
const _EC_LPC_STATUS_SCI_PENDING: u8 = 0x20;
/// SMI event is pending (requesting SMI query)
const _EC_LPC_STATUS_SMI_PENDING: u8 = 0x40;
/// Reserved
const _EC_LPC_STATUS_RESERVED: u8 = 0x80;

/// EC is busy
const EC_LPC_STATUS_BUSY_MASK: u8 = EC_LPC_STATUS_FROM_HOST | EC_LPC_STATUS_PROCESSING;

// I/O addresses for ACPI commands
const _EC_LPC_ADDR_ACPI_DATA: u16 = 0x62;
const _EC_LPC_ADDR_ACPI_CMD: u16 = 0x66;

// I/O addresses for host command
const EC_LPC_ADDR_HOST_DATA: u16 = 0x200;
const EC_LPC_ADDR_HOST_CMD: u16 = 0x204;

// I/O addresses for host command args and params
// Protocol version 2
const EC_LPC_ADDR_HOST_ARGS: u16 = 0x800; /* And 0x801, 0x802, 0x803 */
const _EC_LPC_ADDR_HOST_PARAM: u16 = 0x804; /* For version 2 params; size is
                                             * EC_PROTO2_MAX_PARAM_SIZE */
// Protocol version 3
const _EC_LPC_ADDR_HOST_PACKET: u16 = 0x800; /* Offset of version 3 packet */
const EC_LPC_HOST_PACKET_SIZE: u16 = 0x100; /* Max size of version 3 packet */

const MEC_MEMMAP_OFFSET: u16 = 0x100;
const NPC_MEMMAP_OFFSET: u16 = 0xE00;

// The actual block is 0x800-0x8ff, but some BIOSes think it's 0x880-0x8ff
// and they tell the kernel that so we have to think of it as two parts.
const _EC_HOST_CMD_REGION0: u16 = 0x800;
const _EC_HOST_CMD_REGION1: u16 = 0x8800;
const _EC_HOST_CMD_REGION_SIZE: u16 = 0x80;

// EC command register bit functions
const _EC_LPC_CMDR_DATA: u16 = 1 << 0; // Data ready for host to read
const _EC_LPC_CMDR_PENDING: u16 = 1 << 1; // Write pending to EC
const _EC_LPC_CMDR_BUSY: u16 = 1 << 2; // EC is busy processing a command
const _EC_LPC_CMDR_CMD: u16 = 1 << 3; // Last host write was a command
const _EC_LPC_CMDR_ACPI_BRST: u16 = 1 << 4; // Burst mode (not used)
const _EC_LPC_CMDR_SCI: u16 = 1 << 5; // SCI event is pending
const _EC_LPC_CMDR_SMI: u16 = 1 << 6; // SMI event is pending

const EC_HOST_REQUEST_VERSION: u8 = 3;

/// Request header of version 3
#[repr(C, packed)]
struct EcHostRequest {
    /// Version of this request structure (must be 3)
    pub struct_version: u8,

    /// Checksum of entire request (header and data)
    /// Everything added together adds up to 0 (wrapping around u8 limit)
    pub checksum: u8,

    /// Command number
    pub command: u16,

    /// Command version, usually 0
    pub command_version: u8,

    /// Reserved byte in protocol v3. Must be 0
    pub reserved: u8,

    /// Data length. Data is immediately after the header
    pub data_len: u16,
}

const EC_HOST_RESPONSE_VERSION: u8 = 3;

/// Response header of version 3
#[repr(C, packed)]
struct EcHostResponse {
    /// Version of this request structure (must be 3)
    pub struct_version: u8,

    /// Checksum of entire request (header and data)
    pub checksum: u8,

    /// Status code of response. See enum _EcStatus
    pub result: u16,

    /// Data length. Data is immediately after the header
    pub data_len: u16,

    /// Reserved byte in protocol v3. Must be 0
    pub reserved: u16,
}
#[allow(dead_code)]
pub const HEADER_LEN: usize = std::mem::size_of::<EcHostResponse>();

fn transfer_write(buffer: &[u8]) {
    if has_mec() {
        return portio_mec::transfer_write(buffer);
    }

    if log_enabled!(Level::Trace) {
        print!("transfer_write(size={:#}, buffer=)", buffer.len());
        util::print_buffer(buffer);
    }

    for (i, byte) in buffer.iter().enumerate() {
        Pio::<u8>::new(EC_LPC_ADDR_HOST_ARGS + i as u16).write(*byte);
    }
}

/// Generic transfer read function
fn transfer_read(port: u16, address: u16, size: u16) -> Vec<u8> {
    if has_mec() {
        return portio_mec::transfer_read(address, size);
    }

    if log_enabled!(Level::Trace) {
        println!(
            "transfer_read(port={:#X}, address={:#X}, size={:#X})",
            port, address, size
        );
    }

    // Allocate buffer to hold result
    let mut buffer = vec![0_u8; size.into()];

    for i in 0..size {
        buffer[i as usize] = Pio::<u8>::new(port + address + i).read();
    }

    if log_enabled!(Level::Trace) {
        println!("  Read bytes:");
        util::print_multiline_buffer(&buffer, (port + address) as usize)
    }

    buffer
}

#[cfg(feature = "linux_pio")]
enum Initialized {
    NotYet,
    Succeeded,
    Failed,
}

#[cfg(feature = "linux_pio")]
lazy_static! {
    static ref INITIALIZED: Arc<Mutex<Initialized>> = Arc::new(Mutex::new(Initialized::NotYet));
}

#[cfg(not(feature = "linux_pio"))]
fn init() -> bool {
    // Nothing to do for bare-metal (UEFI) port I/O
    true
}

// In Linux userspace has to first request access to ioports
// TODO: Close these again after we're done
#[cfg(feature = "linux_pio")]
fn init() -> bool {
    let mut init = INITIALIZED.lock().unwrap();
    match *init {
        // Can directly give up, trying again won't help
        Initialized::Failed => return false,
        // Already initialized, no need to do anything.
        Initialized::Succeeded => return true,
        Initialized::NotYet => {}
    }

    if !Uid::effective().is_root() {
        error!("Must be root to use port based I/O for EC communication.");
        *init = Initialized::Failed;
        return false;
    }

    unsafe {
        if has_mec() {
            portio_mec::mec_init();
        } else {
            // 8 for request/response header, 0xFF for response
            let res = ioperm(EC_LPC_ADDR_HOST_ARGS as u64, 8 + 0xFF, 1);
            if res != 0 {
                error!(
                    "ioperm failed. portio driver is likely block by Linux kernel lockdown mode"
                );
                return false;
            }

            let res = ioperm(EC_LPC_ADDR_HOST_CMD as u64, 1, 1);
            assert_eq!(res, 0);
            let res = ioperm(EC_LPC_ADDR_HOST_DATA as u64, 1, 1);
            assert_eq!(res, 0);

            let res = ioperm(
                NPC_MEMMAP_OFFSET as u64,
                (super::EC_MEMMAP_SIZE * 2) as u64,
                1,
            );
            assert_eq!(res, 0);
        }
    }
    *init = Initialized::Succeeded;
    true
}

fn wait_for_ready() {
    if !init() {
        // Failed to initialize
        return;
    }
    // TODO: Abort after reasonable timeout
    loop {
        let status = Pio::<u8>::new(EC_LPC_ADDR_HOST_CMD).read();
        if 0 == (status & EC_LPC_STATUS_BUSY_MASK) {
            break;
        }
        os_specific::sleep(1000)
    }
}

fn checksum_fold(numbers: &[u8]) -> u8 {
    numbers.iter().fold(0u8, |acc, x| acc.wrapping_add(*x))
}

fn checksum_buffers(buffers: &[&[u8]]) -> u8 {
    if log_enabled!(Level::Trace) {
        println!("Checksum of ");
        for buffer in buffers {
            util::print_multiline_buffer(buffer, 0);
        }
    }
    let cs = buffers
        .iter()
        .map(|x| checksum_fold(x))
        .fold(0u8, |acc, x| acc.wrapping_add(x))
        .wrapping_neg();
    if log_enabled!(Level::Trace) {
        println!("  is: {:#X}", cs);
    }

    cs
}
fn checksum_buffer(buffer: &[u8]) -> u8 {
    if log_enabled!(Level::Trace) {
        println!("Checksum of ");
        util::print_multiline_buffer(buffer, 0x00)
    }
    let cs = buffer
        .iter()
        .fold(0u8, |acc, x| acc.wrapping_add(*x))
        .wrapping_neg();
    if log_enabled!(Level::Trace) {
        println!("  is: {:#X}", cs);
    }

    cs
}

fn pack_request(mut request: EcHostRequest, data: &[u8]) -> Vec<u8> {
    let total = EC_LPC_HOST_PACKET_SIZE as usize;
    let offset = std::mem::size_of::<EcHostRequest>();
    let mut buffer = vec![0_u8; total];
    let max_transfer = std::cmp::min(total - offset, data.len());
    let checksum_size = offset + max_transfer;

    if log_enabled!(Level::Trace) {
        println!("Total: {:?}", total);
        println!("Offset: {:?}", offset);
        println!("checksum_size: {:?}", checksum_size);
        println!("data.len(): {:?}", data.len());
    }

    debug_assert!(checksum_size <= total);

    // Copy struct into buffer, then checksum, then copy again
    // Could avoid copying again by inserting checksum in to the buffer directly,
    // but I'm too lazy. And that seems less safe.
    let r_bytes: &[u8] = unsafe { util::any_as_u8_slice(&request) };
    buffer[..offset].copy_from_slice(&r_bytes[..offset]);
    if !data.is_empty() {
        buffer[offset..offset + max_transfer].copy_from_slice(&data[..max_transfer]);
    }
    let checksum = checksum_buffer(&buffer[..checksum_size]);

    request.checksum = checksum;
    let r_bytes: &[u8] = unsafe { util::any_as_u8_slice(&request) };
    buffer[..offset].copy_from_slice(&r_bytes[..offset]);
    if !data.is_empty() {
        buffer[offset..offset + max_transfer].copy_from_slice(&data[..max_transfer]);
    }

    buffer[..checksum_size].to_vec()
}

fn unpack_response_header(bytes: &[u8]) -> EcHostResponse {
    let response: EcHostResponse = unsafe {
        // TODO: Why does transmute not work?
        //std::mem::transmute(bytes.as_ptr())
        std::ptr::read(bytes.as_ptr() as *const _)
    };

    if response.result == 7 {
        println!("Invalid checksum in request!")
    }

    if response.struct_version != 3 {
        println!(
            "Response version is not 0x3! It's {:#X}",
            response.struct_version
        );
    }

    response
}

pub fn send_command(command: u16, command_version: u8, data: &[u8]) -> EcResult<Vec<u8>> {
    if !init() {
        return Err(EcError::DeviceError("Failed to initialize".to_string()));
    }
    let request = EcHostRequest {
        struct_version: EC_HOST_REQUEST_VERSION,
        checksum: 0,
        command,
        // TODO: Has optional dev_index that we could consider
        // rq.command = cec_command->cmd_code |(uint16_t) EC_CMD_PASSTHRU_OFFSET(cec_command->cmd_dev_index);
        command_version,
        reserved: 0,
        data_len: data.len().try_into().unwrap(),
    };
    let request_buffer = pack_request(request, data);

    // Transfer data first, once ready
    if log_enabled!(Level::Trace) {
        println!("Waiting to be ready");
    }
    wait_for_ready();
    if log_enabled!(Level::Trace) {
        print!("Ready, transferring request buffer: ");
    }
    if log_enabled!(Level::Trace) {
        util::print_buffer(&request_buffer);
    }
    transfer_write(&request_buffer);

    // Set the command version
    Pio::<u8>::new(EC_LPC_ADDR_HOST_CMD).write(EC_COMMAND_PROTOCOL_3);
    wait_for_ready();
    let res = Pio::<u8>::new(EC_LPC_ADDR_HOST_DATA).read();
    match FromPrimitive::from_u8(res) {
        None => return Err(EcError::UnknownResponseCode(res as u32)),
        Some(EcResponseStatus::Success) => {}
        Some(status) => return Err(EcError::Response(status)),
    }

    // Read response
    let resp_hdr_buffer = transfer_read(
        EC_LPC_ADDR_HOST_ARGS,
        0,
        std::mem::size_of::<EcHostResponse>() as u16,
    );
    let resp_header = unpack_response_header(&resp_hdr_buffer);
    // TODO: I think we're already covered by checking res above
    // But this seems also to be the EC reponse code, so make sure it's 0 (Success)
    assert_eq!(
        FromPrimitive::from_u16(resp_header.result),
        Some(EcResponseStatus::Success)
    );

    if resp_header.struct_version != EC_HOST_RESPONSE_VERSION {
        return Err(EcError::DeviceError(format!(
            "Struct version invalid. Should be {:#X}, is {:#X}",
            EC_HOST_RESPONSE_VERSION, resp_header.struct_version
        )));
    }
    if resp_header.reserved != 0 {
        return Err(EcError::DeviceError(format!(
            "Reserved invalid. Should be 0, is {:#X}",
            { resp_header.reserved }
        )));
    };
    if log_enabled!(Level::Trace) {
        println!("Data Len is: {:?}", { resp_header.data_len });
    }
    if resp_header.data_len > EC_LPC_HOST_PACKET_SIZE {
        return Err(EcError::DeviceError("Packet size too big".to_string()));
    }
    let resp_buffer = if resp_header.data_len > 0 {
        let data = transfer_read(EC_LPC_ADDR_HOST_ARGS, 8, resp_header.data_len);
        let checksum = checksum_buffers(&[&resp_hdr_buffer, &data]);
        // TODO: probably change to return Err instead
        debug_assert_eq!(checksum, 0);
        data
    } else {
        // Some commands don't return a response body
        vec![]
    };

    // TODO: Check checksum

    Ok(resp_buffer)
}

pub fn read_memory(offset: u16, length: u16) -> EcResult<Vec<u8>> {
    if !init() {
        return Err(EcError::DeviceError("Failed to initialize".to_string()));
    }

    if has_mec() {
        Ok(transfer_read(0, MEC_MEMMAP_OFFSET + offset, length))
    } else {
        Ok(transfer_read(NPC_MEMMAP_OFFSET, offset, length))
    }
}
