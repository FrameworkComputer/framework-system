use core::prelude::rust_2021::derive;
#[cfg(all(not(feature = "uefi"), feature = "std"))]
use std::fs::File;
#[cfg(all(not(feature = "uefi"), feature = "std"))]
use std::io::prelude::*;

use crate::esrt::Guid;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct EfiCapsuleHeader {
    /// A GUID that defines the contents of a capsule.
    pub capsule_guid: Guid,

    /// The size of the capsule header. This may be larger than the size of
    /// the EFI_CAPSULE_HEADER since CapsuleGuid may imply
    /// extended header entries
    pub header_size: u32,

    /// Bit-mapped list describing the capsule attributes. The Flag values
    /// of 0x0000 - 0xFFFF are defined by CapsuleGuid. Flag values
    /// of 0x10000 - 0xFFFFFFFF are defined by this specification
    pub flags: u32,

    /// Size in bytes of the capsule.
    pub capsule_image_size: u32,
}

#[repr(C, packed)]
pub struct DisplayPayload {
    version: u8,
    checksum: u8,
    image_type: u8,
    reserved: u8,
    mode: u32,
    offset_x: u32,
    offset_y: u32,
    //Image[u8, 0];
}

#[repr(C, packed)]
pub struct DisplayCapsule {
    capsule_header: EfiCapsuleHeader,
    image_payload: DisplayPayload,
}

enum ImageType {
    Bitmap = 0,
}

const CAPSULE_FLAGS_PERSIST_ACROSS_RESET: u32 = 0x00010000;
const CAPSULE_FLAGS_POPULATE_SYSTEM_TABLE: u32 = 0x00020000;
const CAPSULE_FLAGS_INITIATE_RESET: u32 = 0x00040000;

fn print_capsule_flags(flags: u32) {
    if flags & CAPSULE_FLAGS_PERSIST_ACROSS_RESET != 0 {
        println!(
            "    Persist across reset  (0x{:x})",
            CAPSULE_FLAGS_PERSIST_ACROSS_RESET
        );
    }
    if flags & CAPSULE_FLAGS_POPULATE_SYSTEM_TABLE != 0 {
        println!(
            "    Populate system table (0x{:x})",
            CAPSULE_FLAGS_POPULATE_SYSTEM_TABLE
        );
    }
    if flags & CAPSULE_FLAGS_INITIATE_RESET != 0 {
        println!(
            "    Initiate reset        (0x{:x})",
            CAPSULE_FLAGS_INITIATE_RESET
        );
    }
}

pub fn parse_capsule_header(data: &[u8]) -> EfiCapsuleHeader {
    let header_len = std::mem::size_of::<EfiCapsuleHeader>();
    let header: EfiCapsuleHeader =
        unsafe { std::ptr::read(data[0..header_len].as_ptr() as *const _) };
    header
}

pub fn print_capsule_header(header: &EfiCapsuleHeader) {
    let header_len = std::mem::size_of::<EfiCapsuleHeader>();
    println!("Capsule Header");
    println!("  Capsule GUID: {}", header.capsule_guid);
    println!("  Header size: {:>19} B", header.header_size);
    if header.header_size as usize > header_len {
        println!("Has extended header entries.");
    }
    println!("  Flags:      {:>20}", format!("0x{:X}", header.flags));
    print_capsule_flags(header.flags);
    println!("  Capsule Size: {:>18} B", header.capsule_image_size);
    println!(
        "  Capsule Size: {:>18} KB",
        header.capsule_image_size / 1024
    );
}

pub fn parse_ux_header(data: &[u8]) -> DisplayCapsule {
    let header_len = std::mem::size_of::<DisplayCapsule>();
    let header: DisplayCapsule =
        unsafe { std::ptr::read(data[0..header_len].as_ptr() as *const _) };
    header
}
pub fn print_ux_header(header: &DisplayCapsule) {
    let header_len = std::mem::size_of::<DisplayCapsule>();
    let ux_header = &header.image_payload;
    println!("Windows UX Header");
    println!("    Version:    {:>20}", ux_header.version);
    // TODO: Check checksum
    // if (CalculateCheckSum8 ((UINT8 *)CapsuleHeader, CapsuleHeader->CapsuleImageSize) != 0) {
    println!("    Checksum:   {:>20}", ux_header.checksum);
    let image_type = if ux_header.image_type == ImageType::Bitmap as u8 {
        " (BMP)"
    } else {
        ""
    }
    .to_string();
    println!("    Image Type: {:>20}{}", ux_header.image_type, image_type);
    println!("    Mode:       {:>20}", { ux_header.mode });
    println!("    Offset X:   {:>20}", { ux_header.offset_x });
    println!("    Offset Y:   {:>20}", { ux_header.offset_y });
    let image_size = header.capsule_header.capsule_image_size as usize - header_len;
    println!("    Calculcated Size: {:>14} B", image_size);
    println!("    Calculcated Size: {:>14} KB", image_size / 1024);
}

#[cfg(all(not(feature = "uefi"), feature = "std"))]
pub fn dump_winux_image(data: &[u8], header: &DisplayCapsule, filename: &str) {
    let header_len = std::mem::size_of::<DisplayCapsule>();
    let image_size = header.capsule_header.capsule_image_size as usize - header_len;

    let image = &data[header_len..image_size];

    let mut file = File::create(filename).unwrap();
    file.write_all(image).unwrap();
}
