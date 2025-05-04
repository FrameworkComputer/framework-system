/*
 * Value written to legacy command port / prefix byte to indicate protocol
 * 3+ structs are being used.  Usage is bus-dependent.
 */
pub const EC_COMMAND_PROTOCOL_3: u8 = 0xda;

// LPC command status byte masks
/// EC has written data but host hasn't consumed it yet
const _EC_LPC_STATUS_TO_HOST: u8 = 0x01;
/// Host has written data/command but EC hasn't consumed it yet
pub const EC_LPC_STATUS_FROM_HOST: u8 = 0x02;
/// EC is still processing a command
pub const EC_LPC_STATUS_PROCESSING: u8 = 0x04;
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
pub const EC_LPC_STATUS_BUSY_MASK: u8 = EC_LPC_STATUS_FROM_HOST | EC_LPC_STATUS_PROCESSING;

// I/O addresses for ACPI commands
const _EC_LPC_ADDR_ACPI_DATA: u16 = 0x62;
const _EC_LPC_ADDR_ACPI_CMD: u16 = 0x66;

// I/O addresses for host command
pub const EC_LPC_ADDR_HOST_DATA: u16 = 0x200;
pub const EC_LPC_ADDR_HOST_CMD: u16 = 0x204;

// I/O addresses for host command args and params
// Protocol version 2
pub const EC_LPC_ADDR_HOST_ARGS: u16 = 0x800; /* And 0x801, 0x802, 0x803 */
const _EC_LPC_ADDR_HOST_PARAM: u16 = 0x804; /* For version 2 params; size is
                                             * EC_PROTO2_MAX_PARAM_SIZE */
// Protocol version 3
const _EC_LPC_ADDR_HOST_PACKET: u16 = 0x800; /* Offset of version 3 packet */
pub const EC_LPC_HOST_PACKET_SIZE: u16 = 0x100; /* Max size of version 3 packet */

pub const MEC_MEMMAP_OFFSET: u16 = 0x100;
pub const NPC_MEMMAP_OFFSET: u16 = 0xE00;

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

pub const EC_HOST_REQUEST_VERSION: u8 = 3;

/// Request header of version 3
#[repr(C, packed)]
pub struct EcHostRequest {
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

pub const EC_HOST_RESPONSE_VERSION: u8 = 3;

/// Response header of version 3
#[repr(C, packed)]
pub struct EcHostResponse {
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
pub const HEADER_LEN: usize = std::mem::size_of::<EcHostResponse>();
