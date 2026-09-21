use alloc::format;
use alloc::string::{String, ToString};
#[cfg(feature = "serde")]
use serde::Serialize;

use super::commands::{BoardIdType, EcResponseDeckState};
use super::{board_id_adc_channel, board_version_table, decode_board_id, CrosEc, EcResult};
use crate::smbios;
use crate::util::PlatformFamily;

/// The number of slots on the input deck, where modules can be connected to
pub const INPUT_DECK_SLOTS: usize = 8;
/// The number of slots on the top row of the input deck
pub const TOP_ROW_SLOTS: usize = 5;

#[repr(u8)]
enum InputDeckMux {
    /// C1 all the way left
    /// B1 all the way left
    /// Keyboard left
    /// Full Width module
    TopRow0 = 0,
    /// C1 2nd-most left
    /// Keyboard middle
    TopRow1,
    /// Keyboard right
    TopRow2,
    /// C1 2nd-most right
    /// B1 all the way right
    TopRow3,
    /// C1 all the way right
    TopRow4,
    /// Touchpad in lower section
    Touchpad,
    /// Pin 6 of the MUX isn't connected to anything
    _Reserved,
    /// The hubboard that all input modules are connected through
    HubBoard = 7,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub enum InputModuleType {
    Short,
    Reserved1,
    Reserved2,
    Reserved3,
    Reserved4,
    Reserved5,
    FullWidth,
    HubBoard,
    GenericA,
    GenericB,
    GenericC,
    KeyboardB,
    KeyboardA,
    Touchpad,
    Reserved15,
    Disconnected,
}
impl From<u8> for InputModuleType {
    fn from(item: u8) -> Self {
        match item {
            0 => Self::Short,
            1 => Self::Reserved1,
            2 => Self::Reserved2,
            3 => Self::Reserved3,
            4 => Self::Reserved4,
            5 => Self::Reserved5,
            6 => Self::FullWidth,
            7 => Self::HubBoard,
            8 => Self::GenericA,
            9 => Self::GenericB,
            10 => Self::GenericC,
            11 => Self::KeyboardB,
            12 => Self::KeyboardA,
            13 => Self::Touchpad,
            14 => Self::Reserved15,
            15 => Self::Disconnected,
            _ => panic!("Invalid module type"),
        }
    }
}
impl InputModuleType {
    /// How wide is the module? The A size isn't exactly 6 wide, but it covers 6 connectors
    ///
    /// So in total, the input deck is 8 wide.
    pub fn size(&self) -> usize {
        match self {
            Self::Short => 0,
            Self::Reserved1 => 0,
            Self::Reserved2 => 0,
            Self::Reserved3 => 0,
            Self::Reserved4 => 0,
            Self::Reserved5 => 0,
            Self::FullWidth => 8,
            Self::HubBoard => 0,
            Self::GenericA => 6,
            Self::GenericB => 2,
            Self::GenericC => 1,
            Self::KeyboardB => 2,
            Self::KeyboardA => 6,
            Self::Touchpad => 0,
            Self::Reserved15 => 0,
            Self::Disconnected => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub enum InputDeckState {
    /// Manual workaround during EVT
    Off,
    /// Input deck not powered on
    Disconnected,
    /// Input deck debounce, waiting to turn on
    TurningOn,
    /// Input deck powered on
    On,
    /// Manual override: Always off
    ForceOff,
    /// Manual override: Always on
    ForceOn,
    /// Manual override: Input deck will follow power sequence, no present check
    NoDetection,
}
impl InputDeckState {
    /// Whether the EC is powering the input modules in this state
    pub fn modules_powered(&self) -> bool {
        matches!(self, InputDeckState::On | InputDeckState::ForceOn)
    }
}

impl From<u8> for InputDeckState {
    fn from(item: u8) -> Self {
        match item {
            0 => InputDeckState::Off,
            1 => InputDeckState::Disconnected,
            2 => InputDeckState::TurningOn,
            3 => InputDeckState::On,
            4 => InputDeckState::ForceOff,
            5 => InputDeckState::ForceOn,
            6 => InputDeckState::NoDetection,
            _ => panic!("Invalid value"),
        }
    }
}

/// Board version that means no board is installed
///
/// See `BOARD_VERSION_NOT_INSTALLED` in the EC. Values above it can't be a
/// board version at all, they're `BOARD_VERSION_UNKNOWN` (-1) truncated to u8,
/// which the EC returns when it can't decode the ADC channel.
const BOARD_VERSION_NOT_INSTALLED: u8 = 15;
/// Lowest touchpad board version that counts as connected, while the input
/// modules are powered
///
/// Mirrors `input_c_deck_detect` in the EC.
const TOUCHPAD_BOARD_VERSION_MIN_POWERED: u8 = 1;
/// Highest touchpad board version that counts as connected, while the input
/// modules are not powered
///
/// Mirrors `input_c_deck_detect` in the EC. An empty connector reads 11.
const TOUCHPAD_BOARD_VERSION_MAX_UNPOWERED: u8 = 10;

/// Whether a Framework 13 touchpad board counts as connected
///
/// On Framework 13 the EC reports the board version of the touchpad board and
/// compares it against a different threshold depending on whether the input
/// modules are currently powered, because the board ID pin is pulled
/// differently in each case.
fn touchpad_present_13(board_version: u8, state: InputDeckState) -> bool {
    if board_version >= BOARD_VERSION_NOT_INSTALLED {
        return false;
    }

    if state.modules_powered() {
        board_version >= TOUCHPAD_BOARD_VERSION_MIN_POWERED
    } else {
        board_version <= TOUCHPAD_BOARD_VERSION_MAX_UNPOWERED
    }
}

/// State of the input deck
///
/// Which fields are filled depends on the platform family, because the EC
/// reports the board ID array differently. Only Framework 16 populates all
/// slots with [`InputModuleType`]. Framework 13 zeroes the array and fills in
/// the board version of the touchpad board at the touchpad position, so the
/// module-type fields would be meaningless there and are left as `None`.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct InputDeckStatus {
    pub state: InputDeckState,
    /// Whether a touchpad is connected
    pub touchpad_present: bool,
    /// Framework 16 only, the module type reported for the touchpad position
    pub touchpad_module: Option<InputModuleType>,
    /// Framework 13 only, the board version of the touchpad board, `None` if
    /// the EC couldn't read it
    pub touchpad_board_version: Option<u8>,
    /// Framework 16 only, whether the hubboard is connected
    pub hubboard_present: Option<bool>,
    /// Framework 16 only, the modules in the five top row positions
    pub top_row: Option<TopRowPositions>,
}

impl InputDeckStatus {
    pub fn top_row_to_array(&self) -> Option<[InputModuleType; TOP_ROW_SLOTS]> {
        let top_row = self.top_row.as_ref()?;
        Some([
            top_row.pos0,
            top_row.pos1,
            top_row.pos2,
            top_row.pos3,
            top_row.pos4,
        ])
    }
    /// Whether the input deck is fully populated
    pub fn fully_populated(&self) -> bool {
        if matches!(self.state, InputDeckState::ForceOn | InputDeckState::On) {
            return false;
        }

        if self.hubboard_present != Some(true) {
            return false;
        }

        if !self.touchpad_present {
            return false;
        }

        self.top_row_fully_populated()
    }

    pub fn top_row_fully_populated(&self) -> bool {
        self.top_row_to_array()
            .map(|top_row| {
                top_row.iter().map(InputModuleType::size).sum::<usize>() == INPUT_DECK_SLOTS
            })
            .unwrap_or(false)
    }

    /// Decode the EC response for a platform family
    ///
    /// The board ID array means different things per family, see
    /// [`InputDeckStatus`].
    pub fn from_response(item: EcResponseDeckState, family: Option<PlatformFamily>) -> Self {
        let state = InputDeckState::from(item.deck_state);
        let tp_raw = item.board_id[InputDeckMux::Touchpad as usize];

        match family {
            // Of the Framework 12 and 13 systems only sakura implements the
            // host command at all. It reports a board version, not a module
            // type, and leaves every other slot zeroed.
            Some(PlatformFamily::Framework12) | Some(PlatformFamily::Framework13) => {
                InputDeckStatus {
                    state,
                    touchpad_present: touchpad_present_13(tp_raw, state),
                    touchpad_module: None,
                    touchpad_board_version: (tp_raw <= BOARD_VERSION_NOT_INSTALLED)
                        .then_some(tp_raw),
                    hubboard_present: None,
                    top_row: None,
                }
            }
            _ => {
                let tp_module = InputModuleType::from(tp_raw);

                InputDeckStatus {
                    state,
                    // The EC only powers the deck when this position reports a
                    // touchpad, anything else is a module in the wrong slot
                    touchpad_present: matches!(tp_module, InputModuleType::Touchpad),
                    touchpad_module: Some(tp_module),
                    touchpad_board_version: None,
                    hubboard_present: Some(matches!(
                        InputModuleType::from(item.board_id[InputDeckMux::HubBoard as usize],),
                        InputModuleType::HubBoard
                    )),
                    top_row: Some(TopRowPositions {
                        pos0: InputModuleType::from(item.board_id[InputDeckMux::TopRow0 as usize]),
                        pos1: InputModuleType::from(item.board_id[InputDeckMux::TopRow1 as usize]),
                        pos2: InputModuleType::from(item.board_id[InputDeckMux::TopRow2 as usize]),
                        pos3: InputModuleType::from(item.board_id[InputDeckMux::TopRow3 as usize]),
                        pos4: InputModuleType::from(item.board_id[InputDeckMux::TopRow4 as usize]),
                    }),
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct TopRowPositions {
    /// C1 all the way left
    /// B1 all the way left
    /// Keyboard left
    /// Full Width module
    pub pos0: InputModuleType,
    /// C1 2nd-most left
    /// Keyboard middle
    pub pos1: InputModuleType,
    /// Keyboard right
    pub pos2: InputModuleType,
    /// C1 2nd-most right
    /// B1 all the way right
    pub pos3: InputModuleType,
    /// C1 all the way right
    pub pos4: InputModuleType,
}

/// A daughterboard connected to the input deck
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct Daughterboard {
    /// Board ID, `None` if the board is not connected
    pub board_id: Option<u8>,
    /// Raw reading of the board ID ADC channel in mV, `None` if it couldn't be read
    pub adc_mv: Option<i32>,
}

impl Daughterboard {
    pub fn present(&self) -> bool {
        self.board_id.is_some()
    }
}

/// Everything we know about the input deck
///
/// Which fields are filled depends on the platform family. Use
/// [`CrosEc::get_inputdeck_status`] to read it and [`print_inputdeck_status`]
/// to show it like the commandline tool does.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct InputDeckInfo {
    /// Platform family the layout was decoded for, `None` if unknown
    pub family: Option<PlatformFamily>,
    /// Whether the chassis intrusion switch reports the chassis as closed
    pub chassis_closed: Option<bool>,
    /// Framework 12 only
    pub power_button_board: Option<Daughterboard>,
    /// Framework 12 and 13
    pub audio_board: Option<Daughterboard>,
    /// Framework 12 and 13
    pub touchpad_board: Option<Daughterboard>,
    /// State of the input deck, `None` if the EC doesn't report it
    pub deck_status: Option<InputDeckStatus>,
    /// Framework 16 only, whether the SLEEP# GPIO is high
    pub sleep_l: Option<bool>,
}

impl CrosEc {
    /// Read a daughterboard's board ID channel
    ///
    /// Reads the channel once and decodes that same reading, so that board_id
    /// and adc_mv can't disagree. They did when the channel was sampled twice:
    /// a disconnected board leaves the pin floating, so the two samples
    /// differed and we reported a board ID next to millivolts it didn't come
    /// from.
    fn daughterboard(&self, board_id_type: BoardIdType) -> Daughterboard {
        let Some(adc_channel) = board_id_adc_channel(board_id_type) else {
            return Daughterboard {
                board_id: None,
                adc_mv: None,
            };
        };
        let table = board_version_table();
        let adc_mv = self.adc_read(adc_channel).ok();
        Daughterboard {
            board_id: adc_mv.and_then(|mv| match decode_board_id(mv, table) {
                Ok(board_id) => board_id,
                Err(err) => {
                    log::debug!("ADC channel {}: {:?}", adc_channel, err);
                    None
                }
            }),
            adc_mv,
        }
    }

    /// Read the state of the input deck and the boards connected to it
    ///
    /// If the platform family is unknown, tries to detect a Framework 16 by
    /// its SLEEP# GPIO and otherwise only reads the generic deck state.
    pub fn get_inputdeck_status(&self) -> EcResult<InputDeckInfo> {
        let family = match smbios::get_family() {
            family @ Some(
                PlatformFamily::Framework12
                | PlatformFamily::Framework13
                | PlatformFamily::Framework16,
            ) => family,
            // If we don't know which platform it is, we can use some heuristics
            // Only Framework Laptop 16 has this GPIO
            _ if self.get_gpio("sleep_l").is_ok() => Some(PlatformFamily::Framework16),
            _ => None,
        };

        let mut info = InputDeckInfo {
            family,
            chassis_closed: None,
            power_button_board: None,
            audio_board: None,
            touchpad_board: None,
            deck_status: None,
            sleep_l: None,
        };

        match family {
            Some(PlatformFamily::Framework12) => {
                info.chassis_closed = Some(!self.get_intrusion_status()?.currently_open);
                info.power_button_board = Some(self.daughterboard(BoardIdType::PowerButtonBoard));
                info.audio_board = Some(self.daughterboard(BoardIdType::AudioBoard));
                info.touchpad_board = Some(self.daughterboard(BoardIdType::Touchpad));
                info.deck_status = self.get_input_deck_status().ok();
            }
            Some(PlatformFamily::Framework13) => {
                info.chassis_closed = Some(!self.get_intrusion_status()?.currently_open);
                info.audio_board = Some(self.daughterboard(BoardIdType::AudioBoard));
                info.touchpad_board = Some(self.daughterboard(BoardIdType::Touchpad));
                info.deck_status = self.get_input_deck_status().ok();
            }
            Some(PlatformFamily::Framework16) => {
                info.chassis_closed = Some(!self.get_intrusion_status()?.currently_open);
                info.deck_status = Some(self.get_input_deck_status()?);
                info.sleep_l = Some(self.get_gpio("sleep_l")?);
            }
            Some(PlatformFamily::FrameworkDesktop) | None => {
                info.deck_status = self.get_input_deck_status().ok();
            }
        }

        Ok(info)
    }
}

/// Format a daughterboard like the commandline tool does
fn format_daughterboard(board: &Daughterboard) -> String {
    if let Some(board_id) = board.board_id {
        format!("Present ({})", board_id)
    } else {
        "Missing".to_string()
    }
}

fn print_daughterboard(label: &str, board: &Daughterboard) {
    println!("  {:<20} {}", label, format_daughterboard(board));
    if let Some(adc) = board.adc_mv {
        println!("    ADC Value          {:04}mV", adc);
    }
}

/// Print the state of the input deck like the commandline tool does
pub fn print_inputdeck_status(info: &InputDeckInfo) {
    match info.family {
        Some(PlatformFamily::Framework16) => {
            if let Some(closed) = info.chassis_closed {
                println!("Chassis Closed:   {}", closed);
            }
            if let Some(status) = &info.deck_status {
                println!("Input Deck State: {:?}", status.state);
                println!("Touchpad present: {}", status.touchpad_present);
            }
            if let Some(sleep_l) = info.sleep_l {
                println!("SLEEP# GPIO high: {}", sleep_l);
            }
            if let Some(top_row) = info.deck_status.as_ref().and_then(|s| s.top_row.as_ref()) {
                println!("Positions:");
                println!("  Pos 0: {:?}", top_row.pos0);
                println!("  Pos 1: {:?}", top_row.pos1);
                println!("  Pos 2: {:?}", top_row.pos2);
                println!("  Pos 3: {:?}", top_row.pos3);
                println!("  Pos 4: {:?}", top_row.pos4);
            }
        }
        Some(PlatformFamily::Framework12) | Some(PlatformFamily::Framework13) => {
            println!("Input Deck");
            if let Some(closed) = info.chassis_closed {
                println!("  Chassis Closed:      {}", closed);
            }
            if let Some(board) = &info.power_button_board {
                print_daughterboard("Power Button Board:", board);
            }
            if let Some(board) = &info.audio_board {
                print_daughterboard("Audio Daughterboard:", board);
            }
            if let Some(board) = &info.touchpad_board {
                print_daughterboard("Touchpad:", board);
            }
            if let Some(status) = &info.deck_status {
                println!("  Deck State:          {:?}", status.state);
                println!("  Touchpad present:    {}", status.touchpad_present);
                if let Some(board_version) = status.touchpad_board_version {
                    println!("    Board Version      {}", board_version);
                }
            }
        }
        Some(PlatformFamily::FrameworkDesktop) | None => {
            if let Some(status) = &info.deck_status {
                println!("  Deck State:          {:?}", status.state);
                if let Some(module) = status.touchpad_module {
                    println!(
                        "  Touchpad present:    {} ({:?})",
                        status.touchpad_present, module
                    );
                } else {
                    println!("  Touchpad present:    {}", status.touchpad_present);
                }
            } else {
                println!("  Unable to tell");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck_response(touchpad: u8, deck_state: u8) -> EcResponseDeckState {
        let mut board_id = [0; INPUT_DECK_SLOTS];
        board_id[InputDeckMux::Touchpad as usize] = touchpad;
        EcResponseDeckState {
            board_id,
            deck_state,
        }
    }

    /// Framework 13 reports the board version of the touchpad board, not a
    /// module type, and zeroes every other slot
    #[test]
    fn decode_deck_state_13() {
        let family = Some(PlatformFamily::Framework13);

        // Touchpad removed. The empty connector reads board version 11 while
        // the modules aren't powered, which the EC treats as not connected.
        let removed = InputDeckStatus::from_response(deck_response(11, 1), family);
        assert_eq!(removed.state, InputDeckState::Disconnected);
        assert!(!removed.touchpad_present);
        assert_eq!(removed.touchpad_board_version, Some(11));

        // Touchpad installed, modules powered, reads board version 7
        let installed = InputDeckStatus::from_response(deck_response(7, 3), family);
        assert_eq!(installed.state, InputDeckState::On);
        assert!(installed.touchpad_present);
        assert_eq!(installed.touchpad_board_version, Some(7));

        // The module type fields are meaningless on Framework 13
        for status in [&removed, &installed] {
            assert_eq!(status.touchpad_module, None);
            assert_eq!(status.hubboard_present, None);
            assert_eq!(status.top_row, None);
            assert!(!status.top_row_fully_populated());
            assert!(!status.fully_populated());
        }

        // Undefined board version, below the powered threshold
        let undefined = InputDeckStatus::from_response(deck_response(0, 3), family);
        assert!(!undefined.touchpad_present);

        // Not installed, and the EC failing to decode the ADC channel, which
        // it reports as BOARD_VERSION_UNKNOWN (-1) truncated to u8
        for (raw, expected) in [(15, Some(15)), (255, None)] {
            let status = InputDeckStatus::from_response(deck_response(raw, 3), family);
            assert!(!status.touchpad_present);
            assert_eq!(status.touchpad_board_version, expected);
        }
    }

    /// Framework 16 reports the type of the connected module in every slot
    #[test]
    fn decode_deck_state_16() {
        let family = Some(PlatformFamily::Framework16);

        let installed = InputDeckStatus::from_response(deck_response(13, 3), family);
        assert!(installed.touchpad_present);
        assert_eq!(installed.touchpad_module, Some(InputModuleType::Touchpad));
        assert_eq!(installed.touchpad_board_version, None);
        assert_eq!(installed.hubboard_present, Some(false));

        // Only a touchpad counts, a module in the wrong slot doesn't. 11 is
        // what a Framework 13 reads with no touchpad connected.
        for raw in [0, 11, 15] {
            let status = InputDeckStatus::from_response(deck_response(raw, 3), family);
            assert!(!status.touchpad_present);
        }

        let mut response = deck_response(13, 3);
        response.board_id[InputDeckMux::HubBoard as usize] = InputModuleType::HubBoard as u8;
        let status = InputDeckStatus::from_response(response, family);
        assert_eq!(status.hubboard_present, Some(true));
        assert_eq!(
            status.top_row_to_array(),
            Some([InputModuleType::Short; TOP_ROW_SLOTS])
        );
    }

    #[test]
    fn format_daughterboards() {
        let present = Daughterboard {
            board_id: Some(7),
            adc_mv: Some(1056),
        };
        assert!(present.present());
        assert_eq!(format_daughterboard(&present), "Present (7)");

        let missing = Daughterboard {
            board_id: None,
            adc_mv: None,
        };
        assert!(!missing.present());
        assert_eq!(format_daughterboard(&missing), "Missing");
    }
}
