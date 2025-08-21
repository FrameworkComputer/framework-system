//! Parse UEFI capsule binaries and extract the metadata
//!
//! UEFI capsule are pretty simple, they start with the headers struct,
//! which includs information about the total size and follows with the data.
//! The data portion is opaque and can be anything. It is interpreted by the
//! appropriate driver, in UEFI during boot, that knows how to handle a capsule
//! with the specified GUID.
//!
//! Currently NOT implemented is parsing capsules with mutiple header structs!

use std::prelude::v1::*;

use core::prelude::rust_2021::derive;
use guid_create::CGuid;
#[cfg(not(feature = "uefi"))]
use std::fs::File;
#[cfg(not(feature = "uefi"))]
use std::io::prelude::*;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct EfiCapsuleHeader {
    /// A GUID that defines the contents of a capsule.
    pub capsule_guid: CGuid,

    /// The size of the capsule header. This may be larger than the size of
    /// the EFI_CAPSULE_HEADER since CapsuleGuid may imply
    /// extended header entries
    pub header_size: u32,

    /// Bit-mapped list describing the capsule attributes. The Flag values
    /// of 0x0000 - 0xFFFF are defined by CapsuleGuid. Flag values
    /// of 0x10000 - 0xFFFFFFFF are defined by this specification
    pub flags: u32,

    /// Size in bytes of the entire capsule, including header.
    pub capsule_image_size: u32,
}

impl EfiCapsuleHeader {
    /// Check if the capsule is valid
    ///
    /// This is very useful to check if the binary data we're parsing is a capsule at all, or not.
    pub fn is_valid(&self, data: &[u8]) -> bool {
        let size = data.len() as u32;
        let header_size = std::mem::size_of::<EfiCapsuleHeader>() as u32;
        if self.capsule_image_size != size {
            return false;
        }
        if self.header_size > self.capsule_image_size {
            return false;
        }
        if self.header_size < header_size {
            return false;
        }

        true
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct DisplayPayload {
    version: u8,
    checksum: u8,
    /// See `ImageType` enum, currently only value 0 is specified
    image_type: u8,
    reserved: u8,
    mode: u32,
    /// Offset in X direction (horizontal, from top left, I thinK?) where the image shall be displayed
    offset_x: u32,
    /// Offset in Y direction (vertical, from top left, I thinK?) where the image shall be displayed
    offset_y: u32,
    //Image[u8, 0];
}

#[derive(PartialEq, Clone, Copy, Debug)]
#[repr(C, packed)]
/// A "display capsule" does not contain firmware, instead it includes an image
/// that shall be displayed during firmware update.
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

pub fn parse_capsule_header(data: &[u8]) -> Option<EfiCapsuleHeader> {
    let header_len = std::mem::size_of::<EfiCapsuleHeader>();
    let header: EfiCapsuleHeader =
        unsafe { std::ptr::read(data[0..header_len].as_ptr() as *const _) };
    if header.is_valid(data) {
        Some(header)
    } else {
        None
    }
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

/// Extract the image data from the display capsule to a file
pub fn dump_winux_image(data: &[u8], header: &DisplayCapsule, filename: &str) {
    let header_len = std::mem::size_of::<DisplayCapsule>();
    let image_size = header.capsule_header.capsule_image_size as usize - header_len;

    let image = &data[header_len..image_size];

    #[cfg(not(feature = "uefi"))]
    {
        let mut file = File::create(filename).unwrap();
        file.write_all(image).unwrap();
    }
    #[cfg(feature = "uefi")]
    {
        let ret = crate::uefi::fs::shell_write_file(filename, image);
        if let Err(err) = ret {
            println!("Failed to dump winux image: {:?}", err);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::esrt;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn can_parse_winux_binary() {
        let mut capsule_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        capsule_path.push("test_bins/winux.bin");

        let data = fs::read(capsule_path).unwrap();
        let cap = parse_capsule_header(&data).unwrap();
        let expected_header = EfiCapsuleHeader {
            capsule_guid: CGuid::from(esrt::WINUX_GUID),
            header_size: 28,
            flags: 65536,
            capsule_image_size: 676898,
        };
        assert_eq!(cap, expected_header);

        assert_eq!(cap.capsule_guid, CGuid::from(esrt::WINUX_GUID));
        let ux_header = parse_ux_header(&data);
        assert_eq!(
            ux_header,
            DisplayCapsule {
                capsule_header: expected_header,
                image_payload: DisplayPayload {
                    version: 1,
                    checksum: 61,
                    image_type: 0,
                    reserved: 0,
                    mode: 0,
                    offset_x: 0,
                    offset_y: 1228,
                }
            }
        );
    }
}
