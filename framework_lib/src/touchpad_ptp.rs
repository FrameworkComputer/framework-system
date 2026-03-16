use std::collections::HashMap;
use std::fs;

use crate::touchpad::PIX_VID;

const DIGITIZER_PAGE: u16 = 0x000D;
const USAGE_INPUT_MODE: u16 = 0x0052;

#[derive(Debug, Clone)]
struct FeatureField {
    report_id: u8,
    bit_offset: usize,
    bit_size: usize,
}

/// Get the current PTP input mode value.
/// Returns 0 for mouse mode, 3 for PTP mode.
pub fn get_ptp_mode() -> Option<u8> {
    let (hidraw_name, field, report_byte_size) = find_touchpad_input_mode()?;

    let hidraw_path = format!("/dev/{}", hidraw_name);
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&hidraw_path)
        .ok()?;

    let mut buf = vec![0u8; 1 + report_byte_size];
    buf[0] = field.report_id;
    hid_get_feature(&file, &mut buf).ok()?;
    Some(extract_bits(&buf[1..], field.bit_offset, field.bit_size) as u8)
}

/// Set the PTP input mode value.
/// Use 0 for mouse mode, 3 for PTP mode.
pub fn set_ptp_mode(mode: u8) -> Option<()> {
    let (hidraw_name, field, report_byte_size) = find_touchpad_input_mode()?;

    let hidraw_path = format!("/dev/{}", hidraw_name);
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&hidraw_path)
        .ok()?;

    // Read-modify-write
    let mut buf = vec![0u8; 1 + report_byte_size];
    buf[0] = field.report_id;
    hid_get_feature(&file, &mut buf).ok()?;
    insert_bits(&mut buf[1..], field.bit_offset, field.bit_size, mode as u32);
    hid_set_feature(&file, &buf).ok()?;
    Some(())
}

/// Parse VID from a hidraw device's uevent file.
/// The HID_ID line has format: HID_ID=BBBB:VVVVVVVV:PPPPPPPP
fn parse_vid_from_uevent(hidraw_name: &str) -> Option<u16> {
    let uevent_path = format!("/sys/class/hidraw/{}/device/uevent", hidraw_name);
    let content = fs::read_to_string(&uevent_path).ok()?;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("HID_ID=") {
            // Format: BBBB:VVVVVVVV:PPPPPPPP
            let parts: Vec<&str> = rest.split(':').collect();
            if parts.len() >= 2 {
                return u32::from_str_radix(parts[1], 16).ok().map(|v| v as u16);
            }
        }
    }
    None
}

/// Find the touchpad's hidraw device by scanning sysfs, then parse its descriptor
/// to locate the Input Mode feature.
/// Returns (hidraw_name, FeatureField, report_byte_size).
fn find_touchpad_input_mode() -> Option<(String, FeatureField, usize)> {
    debug!("Looking for touchpad PTP input mode");

    let hidraw_dir = match fs::read_dir("/sys/class/hidraw") {
        Ok(dir) => dir,
        Err(e) => {
            error!("Failed to read /sys/class/hidraw: {}", e);
            return None;
        }
    };

    for entry in hidraw_dir.flatten() {
        let hidraw_name = entry.file_name().to_string_lossy().to_string();
        if !hidraw_name.starts_with("hidraw") {
            continue;
        }

        let vid = match parse_vid_from_uevent(&hidraw_name) {
            Some(vid) => vid,
            None => continue,
        };

        trace!("  {} VID={:04X}", hidraw_name, vid);

        if vid != PIX_VID {
            continue;
        }

        debug!("  Found PixArt device: {}", hidraw_name);

        // Read the HID report descriptor from sysfs
        let desc_path = format!("/sys/class/hidraw/{}/device/report_descriptor", hidraw_name);
        let desc = match fs::read(&desc_path) {
            Ok(d) => d,
            Err(e) => {
                debug!("  Failed to read report descriptor: {}", e);
                continue;
            }
        };

        let (fields, report_sizes) = parse_ptp_features(&desc);

        if let Some(field) = fields.get(&USAGE_INPUT_MODE) {
            let report_byte_size = report_sizes.get(&field.report_id).copied().unwrap_or(0);
            debug!(
                "  Found Input Mode: report_id={}, bit_offset={}, bit_size={}",
                field.report_id, field.bit_offset, field.bit_size
            );
            return Some((hidraw_name, field.clone(), report_byte_size));
        }
    }

    error!("Could not find touchpad with PTP Input Mode feature");
    None
}

// ── HID feature report I/O via ioctl ─────────────────────────────────────────

// Linux HIDRAW ioctl numbers
// HIDIOCGFEATURE(len) = _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x07, len)
// HIDIOCSFEATURE(len) = _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, len)

fn hid_get_feature(file: &fs::File, buf: &mut [u8]) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();
    let len = buf.len();
    // _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x07, len)
    // direction = 0xC0000000 (WR), type = 'H' (0x48), nr = 0x07, size = len
    let request = 0xC000_0000u32 | ((len as u32 & 0x3FFF) << 16) | ((b'H' as u32) << 8) | 0x07;
    let ret = unsafe { libc::ioctl(fd, request as libc::c_ulong, buf.as_mut_ptr()) };
    if ret < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn hid_set_feature(file: &fs::File, buf: &[u8]) -> std::io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();
    let len = buf.len();
    // _IOC(_IOC_WRITE|_IOC_READ, 'H', 0x06, len)
    let request = 0xC000_0000u32 | ((len as u32 & 0x3FFF) << 16) | ((b'H' as u32) << 8) | 0x06;
    let ret = unsafe { libc::ioctl(fd, request as libc::c_ulong, buf.as_ptr()) };
    if ret < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

// ── Bit manipulation ─────────────────────────────────────────────────────────

fn extract_bits(data: &[u8], bit_offset: usize, bit_size: usize) -> u32 {
    if bit_size == 0 {
        return 0;
    }
    let byte_offset = bit_offset / 8;
    let bit_shift = bit_offset % 8;
    let bytes_needed = (bit_shift + bit_size).div_ceil(8);
    let mut value: u32 = 0;
    for i in 0..bytes_needed {
        if byte_offset + i < data.len() {
            value |= (data[byte_offset + i] as u32) << (i * 8);
        }
    }
    (value >> bit_shift) & ((1u32 << bit_size) - 1)
}

fn insert_bits(data: &mut [u8], bit_offset: usize, bit_size: usize, value: u32) {
    if bit_size == 0 {
        return;
    }
    let byte_offset = bit_offset / 8;
    let bit_shift = bit_offset % 8;
    let mask = ((1u32 << bit_size) - 1) << bit_shift;
    let shifted_value = (value & ((1u32 << bit_size) - 1)) << bit_shift;
    let bytes_needed = (bit_shift + bit_size).div_ceil(8);
    for i in 0..bytes_needed {
        if byte_offset + i < data.len() {
            let byte_mask = (mask >> (i * 8)) as u8;
            let byte_val = (shifted_value >> (i * 8)) as u8;
            data[byte_offset + i] = (data[byte_offset + i] & !byte_mask) | (byte_val & byte_mask);
        }
    }
}

// ── HID descriptor parser ────────────────────────────────────────────────────

/// Parse HID report descriptor to find PTP feature fields.
/// Returns (usage -> FeatureField map, report_id -> report byte size map).
fn parse_ptp_features(desc: &[u8]) -> (HashMap<u16, FeatureField>, HashMap<u8, usize>) {
    let mut fields: HashMap<u16, FeatureField> = HashMap::new();

    // Global state
    let mut usage_page: u16 = 0;
    let mut report_id: u8 = 0;
    let mut report_size: u32 = 0;
    let mut report_count: u32 = 0;

    // Local state (cleared after each main item)
    let mut usages: Vec<u16> = Vec::new();

    // Per-report-id bit offset tracking for Feature reports
    let mut feature_bit_offsets: HashMap<u8, usize> = HashMap::new();

    let mut i = 0;
    while i < desc.len() {
        let prefix = desc[i];

        // Long item
        if prefix == 0xFE {
            if i + 2 >= desc.len() {
                break;
            }
            let data_size = desc[i + 1] as usize;
            i += 3 + data_size;
            continue;
        }

        // Short item
        let size = match prefix & 0x03 {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 4,
            _ => unreachable!(),
        };

        if i + 1 + size > desc.len() {
            break;
        }

        let tag = prefix & 0xFC;
        let data = &desc[i + 1..i + 1 + size];

        match tag {
            // Usage Page (Global)
            0x04 => {
                usage_page = read_unsigned(data, size) as u16;
            }
            // Usage (Local)
            0x08 => {
                usages.push(read_unsigned(data, size) as u16);
            }
            // Report Size (Global)
            0x74 => {
                report_size = read_unsigned(data, size);
            }
            // Report ID (Global)
            0x84 => {
                if let Some(&id) = data.first() {
                    report_id = id;
                }
            }
            // Report Count (Global)
            0x94 => {
                report_count = read_unsigned(data, size);
            }
            // Feature (Main)
            0xB0 => {
                let base_offset = *feature_bit_offsets.entry(report_id).or_insert(0);

                if usage_page == DIGITIZER_PAGE {
                    for field_idx in 0..report_count as usize {
                        let usage = if field_idx < usages.len() {
                            usages[field_idx]
                        } else if !usages.is_empty() {
                            *usages.last().unwrap()
                        } else {
                            continue;
                        };

                        if usage == USAGE_INPUT_MODE {
                            fields.insert(
                                usage,
                                FeatureField {
                                    report_id,
                                    bit_offset: base_offset + field_idx * report_size as usize,
                                    bit_size: report_size as usize,
                                },
                            );
                        }
                    }
                }

                let total_bits = report_count as usize * report_size as usize;
                *feature_bit_offsets.get_mut(&report_id).unwrap() += total_bits;
                usages.clear();
            }
            // Input (Main), Output (Main), Collection (Main) — clear local state
            0x80 | 0x90 | 0xA0 => {
                usages.clear();
            }
            _ => {}
        }

        i += 1 + size;
    }

    // Convert per-report bit totals to byte sizes
    let report_byte_sizes: HashMap<u8, usize> = feature_bit_offsets
        .into_iter()
        .map(|(id, bits)| (id, bits.div_ceil(8)))
        .collect();

    (fields, report_byte_sizes)
}

fn read_unsigned(data: &[u8], size: usize) -> u32 {
    match size {
        1 => data[0] as u32,
        2 => u16::from_le_bytes([data[0], data[1]]) as u32,
        4 => u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_insert_bits() {
        let data = [0b1010_0110, 0b1100_0011];
        assert_eq!(extract_bits(&data, 0, 4), 6);
        assert_eq!(extract_bits(&data, 4, 2), 2);
        assert_eq!(extract_bits(&data, 4, 8), 0b0011_1010);

        let mut buf = [0u8; 2];
        insert_bits(&mut buf, 0, 4, 0b1001);
        assert_eq!(extract_bits(&buf, 0, 4), 0b1001);

        let mut buf = [0xFF, 0xFF];
        insert_bits(&mut buf, 2, 3, 0b010);
        assert_eq!(extract_bits(&buf, 2, 3), 0b010);
        assert_eq!(buf[0] & 0b11, 0b11);
        assert_eq!(buf[0] >> 5, 0b111);
    }

    #[test]
    fn test_parse_ptp_features_input_mode() {
        // Minimal HID descriptor with a Feature report containing Input Mode
        let desc: Vec<u8> = vec![
            0x05, 0x0D, // Usage Page (Digitizer)
            0x09, 0x0E, // Usage (Device Configuration)
            0xA1, 0x01, // Collection (Application)
            0x85, 0x03, //   Report ID (3)
            0x09, 0x52, //   Usage (Input Mode)
            0x15, 0x00, //   Logical Minimum (0)
            0x25, 0x03, //   Logical Maximum (3)
            0x75, 0x02, //   Report Size (2)
            0x95, 0x01, //   Report Count (1)
            0xB1, 0x02, //   Feature (Data,Var,Abs)
            0xC0, // End Collection
        ];

        let (fields, sizes) = parse_ptp_features(&desc);

        let im = fields.get(&USAGE_INPUT_MODE).unwrap();
        assert_eq!(im.report_id, 3);
        assert_eq!(im.bit_offset, 0);
        assert_eq!(im.bit_size, 2);

        // 2 bits = 1 byte
        assert_eq!(sizes[&3], 1);
    }
}
