//! Mock EC driver for fuzzing, only compiled with `--cfg fuzzing` (cargo-fuzz sets it)
//!
//! Instead of talking to hardware, EC responses come from a fuzzer-provided input:
//!
//! - The memory map is a static image, so repeated reads of the same offset agree,
//!   like they would on real hardware.
//! - Host command responses are taken sequentially from a tape. Once the tape runs
//!   out, every command fails, so retry/polling loops terminate.
//! - Optionally a fuzz target can install a command handler, to model stateful
//!   behavior (like flash contents) that a tape can't express.

use std::cell::{Cell, RefCell};

use super::{EcError, EcResponseStatus, EcResult, EC_MEMMAP_SIZE};
use num_traits::FromPrimitive;

/// Maximum response payload of the LPC protocol v3 (256 byte packet - 8 byte header)
const MAX_RESPONSE: usize = 0x100 - 8;

#[derive(Default)]
struct State {
    memmap: Vec<u8>,
    tape: Vec<u8>,
    pos: usize,
}

/// Handles a host command `(command, command_version, request)` instead of the tape.
/// Returning `None` falls through to the tape.
pub type CommandHandler = Box<dyn FnMut(u16, u8, &[u8]) -> Option<EcResult<Vec<u8>>>>;

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static HANDLER: RefCell<Option<CommandHandler>> = const { RefCell::new(None) };
    /// Size of the struct that the caller (`send_command_extra`) expects as response.
    /// The driver can't otherwise know it and would almost always fail that size check.
    static EXPECTED_SIZE: Cell<Option<usize>> = const { Cell::new(None) };
}

/// Install fuzzer input. `memmap` is zero-padded to the full memmap size.
pub fn set_input(memmap: &[u8], tape: &[u8]) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.memmap = memmap.to_vec();
        s.memmap.resize(EC_MEMMAP_SIZE as usize, 0);
        s.tape = tape.to_vec();
        s.pos = 0;
    });
    EXPECTED_SIZE.with(|e| e.set(None));
    HANDLER.with(|h| *h.borrow_mut() = None);
}

/// Install a command handler, see [`CommandHandler`]. Cleared by [`set_input`].
pub fn set_command_handler(handler: CommandHandler) {
    HANDLER.with(|h| *h.borrow_mut() = Some(handler));
}

pub(crate) fn set_expected_size(size: Option<usize>) {
    EXPECTED_SIZE.with(|e| e.set(size));
}

pub(crate) fn read_memory(offset: u16, length: u16) -> EcResult<Vec<u8>> {
    STATE.with(|s| {
        let s = s.borrow();
        let start = offset as usize;
        let end = start + length as usize;
        s.memmap
            .get(start..end)
            .map(|x| x.to_vec())
            .ok_or_else(|| EcError::DeviceError("memmap read out of range".to_string()))
    })
}

pub(crate) fn send_command(command: u16, command_version: u8, data: &[u8]) -> EcResult<Vec<u8>> {
    let expected = EXPECTED_SIZE.with(|e| e.take());
    let handled = HANDLER.with(|h| {
        h.borrow_mut()
            .as_mut()
            .and_then(|handler| handler(command, command_version, data))
    });
    if let Some(res) = handled {
        return res;
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(&ctrl) = s.tape.get(s.pos) else {
            return Err(EcError::DeviceError("fuzz tape exhausted".to_string()));
        };
        s.pos += 1;

        // Control byte:
        //   0x00..=0xBF  success, response of expected size (or ctrl-defined length)
        //   0xC0..=0xDF  success, length from next byte (to hit size mismatch paths)
        //   0xE0..=0xFF  EC returned an error status
        let len = match ctrl {
            0xE0..=0xFF => {
                let code = (ctrl - 0xE0) as u32;
                return Err(match <EcResponseStatus as FromPrimitive>::from_u32(code) {
                    Some(status) => EcError::Response(status),
                    None => EcError::UnknownResponseCode(code),
                });
            }
            0xC0..=0xDF => {
                let Some(&l) = s.tape.get(s.pos) else {
                    return Err(EcError::DeviceError("fuzz tape exhausted".to_string()));
                };
                s.pos += 1;
                l as usize
            }
            _ => expected.unwrap_or(ctrl as usize),
        }
        .min(MAX_RESPONSE);

        // Pad with zeros if the tape runs out mid-response
        let start = s.pos.min(s.tape.len());
        let end = (s.pos + len).min(s.tape.len());
        let mut resp = s.tape[start..end].to_vec();
        resp.resize(len, 0);
        s.pos += len;
        Ok(resp)
    })
}

const PLATFORMS: &[crate::util::Platform] = {
    use crate::util::Platform::*;
    &[
        Framework12IntelGen13,
        Framework12IntelCore3,
        IntelGen11,
        IntelGen12,
        IntelGen13,
        IntelCoreUltra1,
        IntelCoreUltra3,
        Framework13Amd7080,
        Framework13AmdAi300,
        Framework16Amd7080,
        Framework16AmdAi300,
        FrameworkDesktopAmdAiMax300,
        FrameworkDesktopAmdAiMax400,
        UnknownSystem,
    ]
};

/// Split a fuzzer input into platform selector, memmap image and command tape and install it
///
/// Layout: `[platform: u8][memmap: EC_MEMMAP_SIZE bytes][tape: rest]`
pub fn setup(data: &[u8]) -> Option<super::CrosEc> {
    let (&platform, rest) = data.split_first()?;
    let platform = PLATFORMS[platform as usize % PLATFORMS.len()];
    crate::smbios::fuzz_set_platform(Some(platform));

    let split = rest.len().min(EC_MEMMAP_SIZE as usize);
    let (memmap, tape) = rest.split_at(split);
    set_input(memmap, tape);
    Some(super::CrosEc::new())
}
