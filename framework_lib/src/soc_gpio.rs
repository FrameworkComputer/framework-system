//! SoC GPIO control via the Linux GPIO character device.
//!
//! Some Framework platforms wire control lines (e.g. touchscreen enable)
//! to SoC pins that aren't exposed through the EC. We drive those by
//! resolving the pad name in pinctrl-debugfs to a `/dev/gpiochipN` line
//! offset and using libgpiod's v2 character device API.
//!
//! Discovery is intentionally by *hardware pad name* (e.g. "GPP_B_18")
//! rather than chip+offset — gpiochip enumeration order shifts depending
//! on which other GPIO providers (cros-ec, USB-LJCA, etc.) probed first,
//! and the Intel pinctrl HID changes per SoC generation. The pad name
//! is the only identifier guaranteed to stay stable.

use std::fs;
use std::path::PathBuf;

use gpiocdev::line::Value;
use gpiocdev::Request;

/// Locate the `/dev/gpiochipN` and line offset for a given pinctrl pad
/// name (e.g. "GPP_B_18"). Returns `None` if the pad isn't present, the
/// pinctrl debugfs isn't readable (not root / no debugfs), or the pad is
/// firmware-locked.
fn locate_pin(pin_name: &str) -> Option<(PathBuf, u32)> {
    // 1. Find the pinctrl directory whose `pins` file mentions our pad.
    let needle = format!(" ({})", pin_name);
    let entries = match fs::read_dir("/sys/kernel/debug/pinctrl") {
        Ok(e) => e,
        Err(e) => {
            error!(
                "Cannot read /sys/kernel/debug/pinctrl ({}); is debugfs mounted and is the process running as root?",
                e
            );
            return None;
        }
    };

    let mut found: Option<(String, u32)> = None;
    for entry in entries.flatten() {
        let pins_path = entry.path().join("pins");
        let Ok(contents) = fs::read_to_string(&pins_path) else {
            continue;
        };
        for line in contents.lines() {
            if !line.contains(&needle) {
                continue;
            }
            // pinctrl-intel annotates locked pads with " [LOCKED ...]" — see
            // drivers/pinctrl/intel/pinctrl-intel.c:intel_pin_dbg_show().
            if line.contains("[LOCKED") {
                error!(
                    "{} is firmware-locked (PADCFGLOCK); cannot toggle from Linux",
                    pin_name
                );
                return None;
            }
            // Format: "pin <N> (<NAME>) ..."
            let off = line
                .split_whitespace()
                .nth(1)
                .and_then(|t| t.parse::<u32>().ok());
            if let Some(off) = off {
                let pctl = entry.file_name().to_string_lossy().into_owned();
                found = Some((pctl, off));
                break;
            }
        }
        if found.is_some() {
            break;
        }
    }

    let (pctl_name, offset) = match found {
        Some(v) => v,
        None => {
            error!("pad {} not found in pinctrl debugfs", pin_name);
            return None;
        }
    };

    // 2. Map pinctrl device name (e.g. "INTC10BC:04") -> /dev/gpiochipN.
    //    /sys/bus/gpio/devices/gpiochipN is itself a symlink whose target
    //    lives under the parent platform/ACPI device, e.g.
    //        gpiochip4 -> ../../../devices/platform/INTC10BC:04/gpiochip4
    //    so canonicalising the entry itself reveals the controller.
    //    `firmware_node` is a more semantic alternative (it points at the
    //    ACPI handle directly) and we fall back to it if the canonical
    //    parent walk somehow doesn't include the controller name.
    let dir = match fs::read_dir("/sys/bus/gpio/devices") {
        Ok(d) => d,
        Err(e) => {
            error!("Cannot read /sys/bus/gpio/devices: {}", e);
            return None;
        }
    };
    for entry in dir.flatten() {
        let candidates = [
            fs::canonicalize(entry.path()).ok(),
            fs::read_link(entry.path().join("firmware_node"))
                .ok()
                .map(|p| entry.path().join(p)),
        ];
        let owned = candidates
            .iter()
            .flatten()
            .any(|p| p.to_string_lossy().contains(&pctl_name));
        if owned {
            let chip_name = entry.file_name();
            let chip_path = PathBuf::from(format!("/dev/{}", chip_name.to_string_lossy()));
            return Some((chip_path, offset));
        }
    }

    error!(
        "no /dev/gpiochipN matches pinctrl controller {} (pad {})",
        pctl_name, pin_name
    );
    None
}

/// Drive a SoC pad as an output to the given level. Releases the line on
/// return; Intel pinctrl preserves PADCFG state across release, so the
/// level stays asserted in hardware.
fn drive_pad(pin_name: &str, value: bool) -> Option<()> {
    let (chip, offset) = locate_pin(pin_name)?;
    debug!(
        "Driving {} on {} line {} -> {}",
        pin_name,
        chip.display(),
        offset,
        value as u8
    );

    let level = if value { Value::Active } else { Value::Inactive };
    match Request::builder()
        .on_chip(&chip)
        .with_consumer("framework_tool")
        .with_line(offset)
        .as_output(level)
        .request()
    {
        Ok(req) => {
            // Drop releases the request fd; the kernel keeps PADCFG bits
            // set on Intel pinctrl, so the line stays driven.
            drop(req);
            Some(())
        }
        Err(e) => {
            error!(
                "failed to request {} (line {}): {}",
                chip.display(),
                offset,
                e
            );
            None
        }
    }
}

/// Toggle the touchscreen enable line on Framework Laptop 13
/// (Intel Core Ultra Series 3 / "Sakura"). The touch IC's enable pin is
/// wired to SoC GPP_B18; driving it low disables the IC, high re-enables.
pub fn sakura_touchscreen(enable: bool) -> Option<()> {
    drive_pad("GPP_B_18", enable)
}
