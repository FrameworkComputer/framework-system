use super::commands::EcResponseDeckState;

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
        InputDeckStatus {
            state: InputDeckState::from(item.deck_state),
            hubboard_present: matches!(
                InputModuleType::from(item.board_id[InputDeckMux::HubBoard as usize],),
                InputModuleType::HubBoard
            ),
            touchpad_present: matches!(
                InputModuleType::from(item.board_id[InputDeckMux::Touchpad as usize],),
                InputModuleType::Touchpad
            ),
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
