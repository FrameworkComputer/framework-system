use alloc::format;
use alloc::string::{String, ToString};

use super::commands::EcResponseDeckState;
use super::{CrosEc, EcResult, Framework12Adc, Framework13Adc, FrameworkHx20Hx30Adc};
use crate::smbios;
use crate::util::{Platform, PlatformFamily};

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

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InputDeckStatus {
    pub state: InputDeckState,
    pub hubboard_present: bool,
    pub touchpad_present: bool,
    pub touchpad_id: u8,
    pub top_row: TopRowPositions,
}

impl InputDeckStatus {
    pub fn top_row_to_array(&self) -> [InputModuleType; TOP_ROW_SLOTS] {
        [
            self.top_row.pos0,
            self.top_row.pos1,
            self.top_row.pos2,
            self.top_row.pos3,
            self.top_row.pos4,
        ]
    }
    /// Whether the input deck is fully populated
    pub fn fully_populated(&self) -> bool {
        if matches!(self.state, InputDeckState::ForceOn | InputDeckState::On) {
            return false;
        }

        if !self.hubboard_present {
            return false;
        }

        if !self.touchpad_present {
            return false;
        }

        self.top_row_fully_populated()
    }

    pub fn top_row_fully_populated(&self) -> bool {
        self.top_row_to_array()
            .iter()
            .map(InputModuleType::size)
            .sum::<usize>()
            == INPUT_DECK_SLOTS
    }
}

impl From<EcResponseDeckState> for InputDeckStatus {
    fn from(item: EcResponseDeckState) -> Self {
        let tp_id = InputModuleType::from(item.board_id[InputDeckMux::Touchpad as usize]);
        let tp_present = !matches!(
            tp_id,
            InputModuleType::Short | InputModuleType::Disconnected
        );

        InputDeckStatus {
            state: InputDeckState::from(item.deck_state),
            hubboard_present: matches!(
                InputModuleType::from(item.board_id[InputDeckMux::HubBoard as usize],),
                InputModuleType::HubBoard
            ),
            touchpad_present: tp_present,
            touchpad_id: item.board_id[InputDeckMux::Touchpad as usize],
            top_row: TopRowPositions {
                pos0: InputModuleType::from(item.board_id[InputDeckMux::TopRow0 as usize]),
                pos1: InputModuleType::from(item.board_id[InputDeckMux::TopRow1 as usize]),
                pos2: InputModuleType::from(item.board_id[InputDeckMux::TopRow2 as usize]),
                pos3: InputModuleType::from(item.board_id[InputDeckMux::TopRow3 as usize]),
                pos4: InputModuleType::from(item.board_id[InputDeckMux::TopRow4 as usize]),
            },
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
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
    fn daughterboard(&self, board_id: Option<u8>, adc_channel: u8) -> Daughterboard {
        Daughterboard {
            board_id,
            adc_mv: self.adc_read(adc_channel).ok(),
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
                let pwrbtn = self.read_board_id_npc_db(Framework12Adc::PowerButtonBoardId as u8)?;
                let audio = self.read_board_id_npc_db(Framework12Adc::AudioBoardId as u8)?;
                let tp = self.read_board_id_npc_db(Framework12Adc::TouchpadBoardId as u8)?;
                info.power_button_board =
                    Some(self.daughterboard(pwrbtn, Framework12Adc::PowerButtonBoardId as u8));
                info.audio_board =
                    Some(self.daughterboard(audio, Framework12Adc::AudioBoardId as u8));
                info.touchpad_board =
                    Some(self.daughterboard(tp, Framework12Adc::TouchpadBoardId as u8));
                info.deck_status = self.get_input_deck_status().ok();
            }
            Some(PlatformFamily::Framework13) => {
                info.chassis_closed = Some(!self.get_intrusion_status()?.currently_open);
                let (audio, tp) = match smbios::get_platform() {
                    Some(Platform::IntelGen11)
                    | Some(Platform::IntelGen12)
                    | Some(Platform::IntelGen13) => (
                        self.read_board_id(FrameworkHx20Hx30Adc::AudioBoardId as u8)?,
                        self.read_board_id(FrameworkHx20Hx30Adc::TouchpadBoardId as u8)?,
                    ),

                    _ => (
                        self.read_board_id_npc_db(Framework13Adc::AudioBoardId as u8)?,
                        self.read_board_id_npc_db(Framework13Adc::TouchpadBoardId as u8)?,
                    ),
                };
                // TODO: On Intel 11th-13th Gen the ADC channels differ, the
                // raw reading below comes from the wrong channel there
                info.audio_board =
                    Some(self.daughterboard(audio, Framework13Adc::AudioBoardId as u8));
                info.touchpad_board =
                    Some(self.daughterboard(tp, Framework13Adc::TouchpadBoardId as u8));
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
            if let Some(status) = &info.deck_status {
                println!("Positions:");
                println!("  Pos 0: {:?}", status.top_row.pos0);
                println!("  Pos 1: {:?}", status.top_row.pos1);
                println!("  Pos 2: {:?}", status.top_row.pos2);
                println!("  Pos 3: {:?}", status.top_row.pos3);
                println!("  Pos 4: {:?}", status.top_row.pos4);
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
            }
        }
        Some(PlatformFamily::FrameworkDesktop) | None => {
            if let Some(status) = &info.deck_status {
                println!("  Deck State:          {:?}", status.state);
                println!(
                    "  Touchpad present:    {} ({})",
                    status.touchpad_present, status.touchpad_id
                );
            } else {
                println!("  Unable to tell");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
