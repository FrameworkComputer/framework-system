use std::error::Error;
use std::fs::File;
use std::os::unix::io::AsRawFd;

use nix::ioctl_readwrite;

// Constants from the NVMe specification for the Identify Controller data structure.
const IDENTIFY_BUFFER_SIZE: usize = 4096;
const MODEL_NUMBER_OFFSET: usize = 24;
const MODEL_NUMBER_LEN: usize = 40;
const FIRMWARE_REV_OFFSET: usize = 64;
const FIRMWARE_REV_LEN: usize = 8;

// NVMe Admin Command opcodes and parameters.
const NVME_ADMIN_IDENTIFY_OPCODE: u8 = 0x06;
const NVME_IDENTIFY_CNS_CTRL: u32 = 0x01; // CNS value for "Identify Controller"

#[repr(C)]
#[derive(Debug, Default)]
struct NvmeAdminCmd {
    opcode: u8,
    flags: u8,
    rsvd1: u16,
    nsid: u32,
    cdw2: u32,
    cdw3: u32,
    metadata: u64,
    addr: u64,
    metadata_len: u32,
    data_len: u32,
    cdw10: u32,
    cdw11: u32,
    cdw12: u32,
    cdw13: u32,
    cdw14: u32,
    cdw15: u32,
    timeout_ms: u32,
    result: u32,
}

ioctl_readwrite!(nvme_admin_cmd_ioctl, b'N', 0x41, NvmeAdminCmd);

/// A struct to hold the retrieved device information.
#[derive(Debug)]
pub struct NvmeInfo {
    pub model_number: String,
    pub firmware_version: String,
}

fn parse_string(buffer: &[u8], offset: usize, len: usize) -> String {
    let bytes = &buffer[offset..offset + len];
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .trim()
        .to_string()
}

/// Sends an NVMe Identify Controller command and returns the firmware version.
pub fn get_nvme_firmware_version(device_path: &str) -> Result<NvmeInfo, Box<dyn Error>> {
    let file = File::open(device_path)
        .map_err(|e| format!("Failed to open NVMe device {}: {}", device_path, e))?;
    let fd = file.as_raw_fd();

    let mut buffer = vec![0u8; IDENTIFY_BUFFER_SIZE];
    let mut cmd = NvmeAdminCmd {
        opcode: NVME_ADMIN_IDENTIFY_OPCODE,
        addr: buffer.as_mut_ptr() as u64,
        data_len: buffer.len() as u32,
        cdw10: NVME_IDENTIFY_CNS_CTRL,
        ..Default::default()
    };

    let status = unsafe { nvme_admin_cmd_ioctl(fd, &mut cmd)? };
    if status != 0 {
        return Err(format!("NVMe command failed with status code: {:#x}", status).into());
    }

    let model_number = parse_string(&buffer, MODEL_NUMBER_OFFSET, MODEL_NUMBER_LEN);
    let firmware_version = parse_string(&buffer, FIRMWARE_REV_OFFSET, FIRMWARE_REV_LEN);

    Ok(NvmeInfo {
        model_number,
        firmware_version,
    })
}
