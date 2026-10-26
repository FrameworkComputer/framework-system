//! Intel HEX loader for the keyboard controller's firmware images

use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use super::{ErrorKind, Stm32KbdError, Stm32KbdResult};

// dahlia_tp_module_common.dtsi partition map of the 64 KB part:
//   0x0000..0xdfbf  code
//   0xdfc0..0xdfff  misc-flags - 8 slots of one doubleword each
//   0xe000..0xffff  storage - Zephyr NVS
//
// A slot reads all zeros when SET and all ones when CLEAR: a mass erase
// precedes every write and leaves flash all ones, and programming only clears
// bits. Slot 0 is update protect; 1..7 are unassigned, leave them erased.
pub const FLASH_BASE: u32 = 0x0800_0000;
pub const FLASH_END: u32 = 0x0801_0000;
/// First address past the code partition
pub const MISC_FLAGS_BASE: u32 = 0x0800_DFC0;
pub const MISC_SLOT_SIZE: u32 = 8;
pub const MISC_SLOT_NO_AUTO_UPDATE: u32 = 0;
pub const STORAGE_BASE: u32 = 0x0800_E000;
/// STM32U073/U083 erase unit
pub const PAGE_SIZE: u32 = 2048;

/// HID product IDs of decks whose images we can read the version out of.
/// Append, do not replace.
const KNOWN_PIDS: [u16; 2] = [0x0038, 0x003E];

/// A firmware image as one contiguous block, gaps filled with 0xFF
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexImage {
    pub base: u32,
    pub bytes: Vec<u8>,
}

fn bad_image(msg: alloc::string::String) -> Stm32KbdError {
    Stm32KbdError::new(ErrorKind::BadImage, msg)
}

fn hex_byte(s: &[u8]) -> Option<u8> {
    let s = core::str::from_utf8(s).ok()?;
    u8::from_str_radix(s, 16).ok()
}

impl HexImage {
    /// Parse the contents of a .hex file.
    ///
    /// Rejects records outside the flash window and anything that would land
    /// in the misc-flags or settings partitions.
    pub fn parse(text: &str) -> Stm32KbdResult<Self> {
        let mut upper: u32 = 0;
        let mut data: Vec<(u32, u8)> = vec![];

        for (i, line) in text.lines().enumerate() {
            let lineno = i + 1;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some(rec) = line.strip_prefix(':') else {
                return Err(bad_image(format!(
                    "line {}: record does not start with ':'",
                    lineno
                )));
            };
            if rec.len() < 10 || rec.len() % 2 != 0 {
                return Err(bad_image(format!("line {}: bad record length", lineno)));
            }
            let Some(raw) = rec
                .as_bytes()
                .chunks(2)
                .map(hex_byte)
                .collect::<Option<Vec<u8>>>()
            else {
                return Err(bad_image(format!("line {}: non-hex character", lineno)));
            };
            if raw.iter().fold(0u8, |acc, b| acc.wrapping_add(*b)) != 0 {
                return Err(bad_image(format!("line {}: checksum mismatch", lineno)));
            }
            let byte_count = raw[0] as usize;
            let offset = u16::from_be_bytes([raw[1], raw[2]]);
            let rec_type = raw[3];
            if byte_count + 5 != raw.len() {
                return Err(bad_image(format!(
                    "line {}: byte count does not match record",
                    lineno
                )));
            }
            let payload = &raw[4..4 + byte_count];

            match rec_type {
                0x00 => {
                    let abs = (upper << 16) | offset as u32;
                    for (i, b) in payload.iter().enumerate() {
                        data.push((abs.wrapping_add(i as u32), *b));
                    }
                }
                0x01 => break,
                // Zephyr/objcopy emit type 04 for 32-bit targets,
                // so seeing this means the file is not one of ours.
                0x02 => {
                    return Err(bad_image(format!(
                        "line {}: segment-address records (type 02) are not supported; expected a 32-bit linear HEX file",
                        lineno
                    )))
                }
                0x04 => {
                    if byte_count != 2 {
                        return Err(bad_image(format!(
                            "line {}: type 04 record has wrong length",
                            lineno
                        )));
                    }
                    upper = u16::from_be_bytes([payload[0], payload[1]]) as u32;
                }
                // Start linear address, informational
                0x05 => {}
                other => {
                    return Err(bad_image(format!(
                        "line {}: unsupported record type 0x{:02X}",
                        lineno, other
                    )))
                }
            }
        }

        let (Some(min_addr), Some(max_addr)) = (
            data.iter().map(|(a, _)| *a).min(),
            data.iter().map(|(a, _)| *a).max(),
        ) else {
            return Err(bad_image("file contains no data records".to_string()));
        };
        if min_addr < FLASH_BASE || max_addr >= FLASH_END {
            return Err(bad_image(format!(
                "image spans 0x{:08X}-0x{:08X}, outside the flash window 0x{:08X}-0x{:08X}",
                min_addr, max_addr, FLASH_BASE, FLASH_END
            )));
        }
        // The code partition ends at misc-flags. An image built for this
        // board never reaches it; one that does is the wrong file, or was
        // built without CONFIG_USE_DT_CODE_PARTITION and has grown over the
        // flag slots and NVS.
        if max_addr >= MISC_FLAGS_BASE {
            return Err(bad_image(format!(
                "image reaches 0x{:08X}, past the end of the code partition (0x{:08X}). Wrong file, or built without CONFIG_USE_DT_CODE_PARTITION?",
                max_addr, MISC_FLAGS_BASE
            )));
        }

        let mut bytes = vec![0xFF; (max_addr - min_addr) as usize + 1];
        for (addr, b) in data {
            bytes[(addr - min_addr) as usize] = b;
        }
        Ok(Self {
            base: min_addr,
            bytes,
        })
    }

    /// Address of the last byte
    pub fn end(&self) -> u32 {
        self.base + self.bytes.len() as u32 - 1
    }

    /// Find the firmware's own version in the image.
    ///
    /// It reaches the host as the HID descriptor's wVersionID, which the
    /// driver copies from its devicetree config (struct i2c_hid_target_config).
    /// Those fields are consecutive u16 with no padding:
    ///
    /// ```text
    /// hid_desc_register  0x0001
    /// vid                0x32AC   (Framework)
    /// pid                varies by deck
    /// version_id         <- the one we want
    /// max_input_length
    /// max_output_length
    /// report_desc_length
    /// ```
    ///
    /// A match only counts if it occurs exactly once and the three lengths
    /// after it are plausible. Ambiguity is worse than not knowing, so None
    /// is returned in that case. Use [`format_version`] to display it.
    pub fn find_version(&self) -> Option<u16> {
        const ANCHOR: [u8; 4] = [0x01, 0x00, 0xAC, 0x32];
        let rd16 = |p: &[u8], i: usize| u16::from_le_bytes([p[i], p[i + 1]]);

        let mut hits = self.bytes.windows(ANCHOR.len() + 10).filter(|w| {
            w[..4] == ANCHOR
                && KNOWN_PIDS.contains(&rd16(w, 4))
                && (8..=64).contains(&rd16(w, 8))
                && (8..=64).contains(&rd16(w, 10))
                && (16..=512).contains(&rd16(w, 12))
        });
        let hit = hits.next()?;
        if hits.next().is_some() {
            return None;
        }
        Some(rd16(hit, 6))
    }
}

/// Format a nibble-encoded version: 0x1234 displays as "12.3.4"
pub fn format_version(version: u16) -> alloc::string::String {
    format!(
        "{}.{}.{}",
        version >> 8,
        (version >> 4) & 0xF,
        version & 0xF
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    /// Build a data record with a correct checksum
    fn record(offset: u16, rec_type: u8, data: &[u8]) -> String {
        let mut raw = vec![
            data.len() as u8,
            (offset >> 8) as u8,
            offset as u8,
            rec_type,
        ];
        raw.extend_from_slice(data);
        let sum = raw.iter().fold(0u8, |acc, b| acc.wrapping_add(*b));
        raw.push(sum.wrapping_neg());
        let mut s = String::from(":");
        for b in raw {
            s.push_str(&format!("{:02X}", b));
        }
        s.push('\n');
        s
    }

    fn file(records: &[String]) -> String {
        let mut s = record(0, 0x04, &[0x08, 0x00]);
        for r in records {
            s.push_str(r);
        }
        s.push_str(&record(0, 0x01, &[]));
        s
    }

    #[test]
    fn parse_fills_gaps() {
        let text = file(&[
            record(0x0000, 0x00, &[1, 2, 3, 4]),
            record(0x0008, 0x00, &[5, 6]),
        ]);
        let img = HexImage::parse(&text).unwrap();
        assert_eq!(img.base, 0x0800_0000);
        assert_eq!(img.bytes, [1, 2, 3, 4, 0xFF, 0xFF, 0xFF, 0xFF, 5, 6]);
        assert_eq!(img.end(), 0x0800_0009);
    }

    #[test]
    fn parse_crlf_and_start_address() {
        let text = file(&[
            record(0x0100, 0x00, &[0xAA, 0xBB]),
            record(0, 0x05, &[0x08, 0x00, 0x01, 0x00]),
        ])
        .replace('\n', "\r\n");
        let img = HexImage::parse(&text).unwrap();
        assert_eq!(img.base, 0x0800_0100);
        assert_eq!(img.bytes, [0xAA, 0xBB]);
    }

    #[test]
    fn parse_rejects_bad_checksum() {
        let mut text = file(&[record(0, 0x00, &[1, 2, 3, 4])]);
        // Flip the last data byte of the data record
        text = text.replacen(":0400000001020304", ":0400000001020305", 1);
        let err = HexImage::parse(&text).unwrap_err();
        assert_eq!(err.kind, ErrorKind::BadImage);
        assert!(err.message.contains("checksum"), "{}", err.message);
    }

    #[test]
    fn parse_rejects_byte_count_mismatch() {
        // Byte count says 2, record carries 4, checksum still adds up
        let raw = [2u8, 0, 0, 0, 1, 2, 3, 4];
        let sum = raw.iter().fold(0u8, |acc, b| acc.wrapping_add(*b));
        let mut rec = String::from(":");
        for b in raw.iter().chain(core::iter::once(&sum.wrapping_neg())) {
            rec.push_str(&format!("{:02X}", b));
        }
        rec.push('\n');
        let err = HexImage::parse(&file(&[rec])).unwrap_err();
        assert!(err.message.contains("byte count"), "{}", err.message);
    }

    #[test]
    fn parse_rejects_segment_records() {
        let text = record(0, 0x02, &[0x10, 0x00]) + &record(0, 0x00, &[1]);
        let err = HexImage::parse(&text).unwrap_err();
        assert!(err.message.contains("type 02"), "{}", err.message);
    }

    #[test]
    fn parse_rejects_outside_flash() {
        // No type 04 record, so this lands at address 0
        let text = record(0, 0x00, &[1, 2]) + &record(0, 0x01, &[]);
        let err = HexImage::parse(&text).unwrap_err();
        assert!(err.message.contains("outside the flash"), "{}", err.message);
    }

    #[test]
    fn parse_rejects_misc_flags_and_nvs() {
        let text = file(&[record(0xDFC0, 0x00, &[0])]);
        let err = HexImage::parse(&text).unwrap_err();
        assert!(err.message.contains("code partition"), "{}", err.message);

        let text = file(&[record(0xDFBF, 0x00, &[0])]);
        assert!(HexImage::parse(&text).is_ok());
    }

    #[test]
    fn parse_rejects_empty() {
        let err = HexImage::parse(&file(&[])).unwrap_err();
        assert!(err.message.contains("no data"), "{}", err.message);
    }

    fn hid_config(pid: u16, version: u16) -> Vec<u8> {
        [0x0001, 0x32AC, pid, version, 36, 36, 120]
            .iter()
            .flat_map(|v: &u16| v.to_le_bytes())
            .collect()
    }

    #[test]
    fn find_version_once() {
        let mut bytes = vec![0u8; 100];
        bytes.extend(hid_config(0x0038, 0x0100));
        bytes.extend([0u8; 20]);
        let img = HexImage {
            base: FLASH_BASE,
            bytes,
        };
        assert_eq!(img.find_version(), Some(0x0100));
        assert_eq!(format_version(0x0100), "1.0.0");
        assert_eq!(format_version(0x1234), "18.3.4");
    }

    #[test]
    fn find_version_rejects_unknown_pid_and_ambiguity() {
        let img = HexImage {
            base: FLASH_BASE,
            bytes: hid_config(0x1234, 0x0100),
        };
        assert_eq!(img.find_version(), None);

        let mut bytes = hid_config(0x0038, 0x0100);
        bytes.extend(hid_config(0x003E, 0x0200));
        let img = HexImage {
            base: FLASH_BASE,
            bytes,
        };
        assert_eq!(img.find_version(), None);
    }
}
