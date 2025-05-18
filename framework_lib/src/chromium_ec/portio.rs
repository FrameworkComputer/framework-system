use crate::chromium_ec::{EcError, EcResponseStatus, EcResult};
use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::convert::TryInto;
#[cfg(not(windows))]
use hwio::{Io, Pio};
#[cfg(target_os = "linux")]
use libc::ioperm;
use log::Level;
#[cfg(target_os = "linux")]
use nix::unistd::Uid;
use num::FromPrimitive;
use spin::Mutex;

use crate::chromium_ec::protocol::*;
use crate::chromium_ec::{portio_mec, EC_MEMMAP_ID};
use crate::os_specific;
use crate::util;

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

#[derive(PartialEq)]
#[allow(dead_code)]
enum Initialized {
    NotYet,
    SucceededMec,
    Succeeded,
    Failed,
}

lazy_static! {
    static ref INITIALIZED: Mutex<Initialized> = Mutex::new(Initialized::NotYet);
}

fn has_mec() -> bool {
    let init = INITIALIZED.lock();
    *init != Initialized::Succeeded
}

fn init() -> bool {
    let mut init = INITIALIZED.lock();
    match *init {
        // Can directly give up, trying again won't help
        Initialized::Failed => return false,
        // Already initialized, no need to do anything.
        Initialized::Succeeded | Initialized::SucceededMec => return true,
        Initialized::NotYet => {}
    }

    // In Linux userspace has to first request access to ioports
    // TODO: Close these again after we're done
    #[cfg(target_os = "linux")]
    if !Uid::effective().is_root() {
        error!("Must be root to use port based I/O for EC communication.");
        *init = Initialized::Failed;
        return false;
    }

    // First try on MEC
    if !portio_mec::init() {
        *init = Initialized::Failed;
        return false;
    }
    let ec_id = portio_mec::transfer_read(MEC_MEMMAP_OFFSET + EC_MEMMAP_ID, 2);
    if ec_id[0] == b'E' && ec_id[1] == b'C' {
        *init = Initialized::SucceededMec;
        return true;
    }

    #[cfg(target_os = "linux")]
    unsafe {
        // 8 for request/response header, 0xFF for response
        let res = ioperm(EC_LPC_ADDR_HOST_ARGS as u64, 8 + 0xFF, 1);
        if res != 0 {
            error!("ioperm failed. portio driver is likely block by Linux kernel lockdown mode");
            *init = Initialized::Failed;
            return false;
        }

        let res = ioperm(EC_LPC_ADDR_HOST_CMD as u64, 1, 1);
        assert_eq!(res, 0);
        let res = ioperm(EC_LPC_ADDR_HOST_DATA as u64, 1, 1);
        assert_eq!(res, 0);

        let res = ioperm(NPC_MEMMAP_OFFSET as u64, super::EC_MEMMAP_SIZE as u64, 1);
        assert_eq!(res, 0);
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
