//! SSD Expansion Card firmware detection
//!
//! Firmware Versions
//! | Flash Die    | Firmware Version |
//! |--------------|------------------|
//! | Hynix V6     | UHFM00.x         |
//! | Micron N28   | UHFM10.x         |
//! | Micron N48   | UHFM30.x         |
//! | Kioxia BiSC6 | UHFM90.x         |
//!
//! On Linux: sudo smartctl -ji /dev/sda | jq -r .firmware_version
//!   Need to install smartmontools
//! On Windows:
//!   winget install --id=smartmontools.smartmontools -e
//!   winget install --id=jqlang.jq  -e
//!   Or use the native Windows API implementation below - written by Claude Code.
//!   Replace this, once the smartmontools library is ready to use.

#[allow(unused_imports)]
use windows::{
    core::*, Win32::Foundation::*, Win32::Storage::FileSystem::*, Win32::System::Ioctl::*,
    Win32::System::IO::DeviceIoControl,
};

/// Framework USB Vendor ID
pub const FRAMEWORK_VID: u16 = 0x32AC;

/// Information about a storage device from ATA IDENTIFY
#[derive(Debug)]
pub struct AtaDeviceInfo {
    pub serial_number: String,
    pub firmware_revision: String,
    pub model_number: String,
}

/// Combined info for Framework SSD expansion cards
#[derive(Debug)]
pub struct FrameworkSsdInfo {
    /// Product name from USB descriptor (e.g., "1TB Card")
    pub product_name: String,
    /// Firmware revision from ATA IDENTIFY (e.g., "UHFM00.6")
    pub firmware_revision: String,
    /// Serial number from ATA IDENTIFY
    pub serial_number: String,
}

/// Information about a storage device from STORAGE_DEVICE_DESCRIPTOR
#[derive(Debug)]
pub struct StorageDeviceInfo {
    pub vendor_id: String,
    pub product_id: String,
    pub product_revision: String,
    pub serial_number: String,
    pub bus_type: StorageBusType,
}

/// Storage bus types (from Windows STORAGE_BUS_TYPE enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum StorageBusType {
    Unknown = 0,
    Scsi = 1,
    Atapi = 2,
    Ata = 3,
    Ieee1394 = 4,
    Ssa = 5,
    Fibre = 6,
    Usb = 7,
    Raid = 8,
    IScsi = 9,
    Sas = 10,
    Sata = 11,
    Sd = 12,
    Mmc = 13,
    Virtual = 14,
    FileBackedVirtual = 15,
    Spaces = 16,
    Nvme = 17,
    Scm = 18,
    Ufs = 19,
}

impl From<u32> for StorageBusType {
    fn from(value: u32) -> Self {
        match value {
            0 => StorageBusType::Unknown,
            1 => StorageBusType::Scsi,
            2 => StorageBusType::Atapi,
            3 => StorageBusType::Ata,
            4 => StorageBusType::Ieee1394,
            5 => StorageBusType::Ssa,
            6 => StorageBusType::Fibre,
            7 => StorageBusType::Usb,
            8 => StorageBusType::Raid,
            9 => StorageBusType::IScsi,
            10 => StorageBusType::Sas,
            11 => StorageBusType::Sata,
            12 => StorageBusType::Sd,
            13 => StorageBusType::Mmc,
            14 => StorageBusType::Virtual,
            15 => StorageBusType::FileBackedVirtual,
            16 => StorageBusType::Spaces,
            17 => StorageBusType::Nvme,
            18 => StorageBusType::Scm,
            19 => StorageBusType::Ufs,
            _ => StorageBusType::Unknown,
        }
    }
}

// IOCTL_STORAGE_QUERY_PROPERTY = CTL_CODE(IOCTL_STORAGE_BASE, 0x0500, METHOD_BUFFERED, FILE_ANY_ACCESS)
// IOCTL_STORAGE_BASE = 0x2D
// CTL_CODE(DeviceType, Function, Method, Access) = ((DeviceType) << 16) | ((Access) << 14) | ((Function) << 2) | (Method)
const IOCTL_STORAGE_QUERY_PROPERTY: u32 = (0x2D << 16) | (0x0500 << 2);

// IOCTL_SCSI_PASS_THROUGH = CTL_CODE(IOCTL_SCSI_BASE, 0x0401, METHOD_BUFFERED, FILE_READ_ACCESS | FILE_WRITE_ACCESS)
// IOCTL_SCSI_BASE = 0x04
const IOCTL_SCSI_PASS_THROUGH: u32 = (0x04 << 16) | (0x03 << 14) | (0x0401 << 2);

// ATA commands
const ATA_IDENTIFY_DEVICE: u8 = 0xEC;

// SCSI ATA PASS-THROUGH (12) command
const SCSI_ATA_PASS_THROUGH_12: u8 = 0xA1;

// ATA IDENTIFY data offsets (in words, each word is 2 bytes)
const ATA_IDENT_SERIAL_OFFSET: usize = 10; // Words 10-19 (20 bytes)
const ATA_IDENT_SERIAL_LEN: usize = 10; // 10 words = 20 bytes
const ATA_IDENT_FW_REV_OFFSET: usize = 23; // Words 23-26 (8 bytes)
const ATA_IDENT_FW_REV_LEN: usize = 4; // 4 words = 8 bytes
const ATA_IDENT_MODEL_OFFSET: usize = 27; // Words 27-46 (40 bytes)
const ATA_IDENT_MODEL_LEN: usize = 20; // 20 words = 40 bytes

#[repr(C)]
#[derive(Default)]
struct StoragePropertyQuery {
    property_id: u32,
    query_type: u32,
    additional_parameters: [u8; 1],
}

// Property IDs
const STORAGE_DEVICE_PROPERTY: u32 = 0;

// Query types
const PROPERTY_STANDARD_QUERY: u32 = 0;

#[repr(C)]
#[derive(Debug)]
struct StorageDeviceDescriptor {
    version: u32,
    size: u32,
    device_type: u8,
    device_type_modifier: u8,
    removable_media: u8,
    command_queuing: u8,
    vendor_id_offset: u32,
    product_id_offset: u32,
    product_revision_offset: u32,
    serial_number_offset: u32,
    bus_type: u32,
    raw_properties_length: u32,
    raw_device_properties: [u8; 1],
}

/// SCSI_PASS_THROUGH structure for Windows
#[repr(C)]
#[derive(Debug)]
struct ScsiPassThrough {
    length: u16,
    scsi_status: u8,
    path_id: u8,
    target_id: u8,
    lun: u8,
    cdb_length: u8,
    sense_info_length: u8,
    data_in: u8,
    data_transfer_length: u32,
    timeout_value: u32,
    data_buffer_offset: usize,
    sense_info_offset: u32,
    cdb: [u8; 16],
}

/// Buffer for SCSI pass-through with data
#[repr(C)]
struct ScsiPassThroughWithBuffers {
    spt: ScsiPassThrough,
    sense_buffer: [u8; 32],
    data_buffer: [u8; 512],
}

// SCSI data direction
const SCSI_IOCTL_DATA_IN: u8 = 1;

/// Extract a null-terminated string from a buffer at a given offset
fn extract_string(buffer: &[u8], offset: u32) -> String {
    if offset == 0 {
        return String::new();
    }
    let offset = offset as usize;
    if offset >= buffer.len() {
        return String::new();
    }

    // Find the null terminator
    let end = buffer[offset..]
        .iter()
        .position(|&b| b == 0)
        .map(|pos| offset + pos)
        .unwrap_or(buffer.len());

    String::from_utf8_lossy(&buffer[offset..end])
        .trim()
        .to_string()
}

/// Extract ATA string from IDENTIFY data (byte-swapped pairs)
fn extract_ata_string(data: &[u8], word_offset: usize, word_len: usize) -> String {
    let byte_offset = word_offset * 2;
    let byte_len = word_len * 2;

    if byte_offset + byte_len > data.len() {
        return String::new();
    }

    let mut result = Vec::with_capacity(byte_len);

    // ATA strings are stored with bytes swapped within each word
    for i in 0..word_len {
        let idx = byte_offset + i * 2;
        // Swap bytes within each word
        result.push(data[idx + 1]);
        result.push(data[idx]);
    }

    String::from_utf8_lossy(&result).trim().to_string()
}

/// Query storage device information using Windows IOCTL_STORAGE_QUERY_PROPERTY
///
/// # Arguments
/// * `device_path` - Path to the device, e.g., r"\\.\PhysicalDrive0" or r"\\.\E:"
///
/// # Returns
/// * `Ok(StorageDeviceInfo)` - Device information including bus type
/// * `Err(Error)` - If the query fails
pub fn get_storage_device_info(device_path: &str) -> Result<StorageDeviceInfo> {
    // Open the device
    let path = HSTRING::from(device_path);
    let handle = unsafe {
        CreateFileW(
            &path,
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )?
    };

    // Prepare the query
    let query = StoragePropertyQuery {
        property_id: STORAGE_DEVICE_PROPERTY,
        query_type: PROPERTY_STANDARD_QUERY,
        ..Default::default()
    };

    // Buffer to receive the descriptor
    let mut buffer = [0u8; 1024];
    let mut returned: u32 = 0;

    unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&query as *const _ as *const std::ffi::c_void),
            std::mem::size_of::<StoragePropertyQuery>() as u32,
            Some(buffer.as_mut_ptr() as *mut std::ffi::c_void),
            buffer.len() as u32,
            Some(&mut returned),
            None,
        )?;

        CloseHandle(handle)?;
    }

    if returned < std::mem::size_of::<StorageDeviceDescriptor>() as u32 {
        return Err(Error::new(HRESULT(-1), "Insufficient data returned"));
    }

    // Parse the descriptor
    let descriptor = unsafe { &*(buffer.as_ptr() as *const StorageDeviceDescriptor) };

    let vendor_id = extract_string(&buffer, descriptor.vendor_id_offset);
    let product_id = extract_string(&buffer, descriptor.product_id_offset);
    let product_revision = extract_string(&buffer, descriptor.product_revision_offset);
    let serial_number = extract_string(&buffer, descriptor.serial_number_offset);
    let bus_type = StorageBusType::from(descriptor.bus_type);

    Ok(StorageDeviceInfo {
        vendor_id,
        product_id,
        product_revision,
        serial_number,
        bus_type,
    })
}

/// Check if a device is a Framework USB device by checking if vendor contains "FRMW"
/// This is a simpler check than querying the actual USB VID
pub fn is_framework_usb_device(device_path: &str) -> bool {
    if let Ok(info) = get_storage_device_info(device_path) {
        // Must be USB bus type and have FRMW vendor
        info.bus_type == StorageBusType::Usb && info.vendor_id.contains("FRMW")
    } else {
        false
    }
}

/// Query ATA device information using SCSI ATA Pass-Through
///
/// This sends an ATA IDENTIFY DEVICE command to get the real firmware version
/// from the drive controller, not just the USB bridge info.
///
/// # Arguments
/// * `device_path` - Path to the device, e.g., r"\\.\PhysicalDrive0"
///
/// # Returns
/// * `Ok(AtaDeviceInfo)` - ATA device information including firmware revision
/// * `Err(Error)` - If the query fails
pub fn get_ata_device_info(device_path: &str) -> Result<AtaDeviceInfo> {
    // Open the device with read/write access for SCSI passthrough
    let path = HSTRING::from(device_path);
    let handle = unsafe {
        CreateFileW(
            &path,
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )?
    };

    // Initialize the SCSI pass-through structure
    let mut sptwb = ScsiPassThroughWithBuffers {
        spt: ScsiPassThrough {
            length: std::mem::size_of::<ScsiPassThrough>() as u16,
            scsi_status: 0,
            path_id: 0,
            target_id: 0,
            lun: 0,
            cdb_length: 12,
            sense_info_length: 32,
            data_in: SCSI_IOCTL_DATA_IN,
            data_transfer_length: 512,
            timeout_value: 10,
            data_buffer_offset: std::mem::offset_of!(ScsiPassThroughWithBuffers, data_buffer),
            sense_info_offset: std::mem::offset_of!(ScsiPassThroughWithBuffers, sense_buffer)
                as u32,
            cdb: [0u8; 16],
        },
        sense_buffer: [0u8; 32],
        data_buffer: [0u8; 512],
    };

    // Build ATA PASS-THROUGH (12) CDB for IDENTIFY DEVICE
    // See SAT-4 specification
    sptwb.spt.cdb[0] = SCSI_ATA_PASS_THROUGH_12; // Operation code
    sptwb.spt.cdb[1] = 4 << 1; // Protocol: PIO Data-In (4), no multiple count
    sptwb.spt.cdb[2] = 0x2E; // flags: T_DIR=1 (from device), BYT_BLOK=1, T_LENGTH=2 (sector count)
    sptwb.spt.cdb[3] = 0; // Features
    sptwb.spt.cdb[4] = 1; // Sector count
    sptwb.spt.cdb[5] = 0; // LBA Low
    sptwb.spt.cdb[6] = 0; // LBA Mid
    sptwb.spt.cdb[7] = 0; // LBA High
    sptwb.spt.cdb[8] = 0; // Device
    sptwb.spt.cdb[9] = ATA_IDENTIFY_DEVICE; // Command
    sptwb.spt.cdb[10] = 0; // Reserved
    sptwb.spt.cdb[11] = 0; // Control

    let mut returned: u32 = 0;
    let buffer_size = std::mem::size_of::<ScsiPassThroughWithBuffers>() as u32;

    let result = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_SCSI_PASS_THROUGH,
            Some(&sptwb as *const _ as *const std::ffi::c_void),
            buffer_size,
            Some(&mut sptwb as *mut _ as *mut std::ffi::c_void),
            buffer_size,
            Some(&mut returned),
            None,
        )
    };

    unsafe {
        CloseHandle(handle)?;
    }

    result?;

    // Check SCSI status
    if sptwb.spt.scsi_status != 0 {
        return Err(Error::new(
            HRESULT(-1),
            format!("SCSI command failed with status: {}", sptwb.spt.scsi_status),
        ));
    }

    // Parse ATA IDENTIFY data
    let data = &sptwb.data_buffer;

    let serial_number = extract_ata_string(data, ATA_IDENT_SERIAL_OFFSET, ATA_IDENT_SERIAL_LEN);
    let firmware_revision = extract_ata_string(data, ATA_IDENT_FW_REV_OFFSET, ATA_IDENT_FW_REV_LEN);
    let model_number = extract_ata_string(data, ATA_IDENT_MODEL_OFFSET, ATA_IDENT_MODEL_LEN);

    Ok(AtaDeviceInfo {
        serial_number,
        firmware_revision,
        model_number,
    })
}

/// Get the firmware version of a storage device using ATA passthrough
///
/// # Arguments
/// * `device_path` - Path to the device, e.g., r"\\.\PhysicalDrive0"
///
/// # Returns
/// * `Ok(String)` - Firmware version string (e.g., "UHFM00.1")
/// * `Err(Error)` - If the query fails
pub fn get_firmware_version(device_path: &str) -> Result<String> {
    let info = get_ata_device_info(device_path)?;
    Ok(info.firmware_revision)
}

/// List Framework SSD expansion cards and their firmware info
///
/// Only returns USB-attached devices with Framework vendor ID (FRMW)
pub fn list_framework_ssd_cards() -> Vec<(String, Result<FrameworkSsdInfo>)> {
    let mut results = Vec::new();

    // Try PhysicalDrive0 through PhysicalDrive15
    for i in 0..16 {
        let path = format!(r"\\.\PhysicalDrive{}", i);

        // Get storage info first - need it for product name and filtering
        let storage_info = match get_storage_device_info(&path) {
            Ok(info) => info,
            Err(_) => continue,
        };

        // Only include Framework USB devices
        if storage_info.bus_type != StorageBusType::Usb || !storage_info.vendor_id.contains("FRMW")
        {
            continue;
        }

        // Get ATA info for firmware version
        let info = match get_ata_device_info(&path) {
            Ok(ata) => Ok(FrameworkSsdInfo {
                product_name: format!("{} {}", storage_info.vendor_id, storage_info.product_id),
                firmware_revision: ata.firmware_revision,
                serial_number: ata.serial_number,
            }),
            Err(e) => Err(e),
        };

        results.push((path, info));
    }

    results
}

/// List all physical drives and their ATA info (unfiltered)
pub fn list_storage_devices() -> Vec<(String, Result<AtaDeviceInfo>)> {
    let mut results = Vec::new();

    // Try PhysicalDrive0 through PhysicalDrive15
    for i in 0..16 {
        let path = format!(r"\\.\PhysicalDrive{}", i);

        // First check if drive exists using storage query
        if get_storage_device_info(&path).is_err() {
            continue;
        }

        // Then try to get ATA info
        let info = get_ata_device_info(&path);
        results.push((path, info));
    }

    results
}
