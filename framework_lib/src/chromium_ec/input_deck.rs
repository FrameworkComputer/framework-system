use super::commands::EcResponseDeckState;

/// The number of slots on the input deck, where modules can be connected to
pub const INPUT_DECK_SLOTS: usize = 8;

#[repr(u8)]
enum InputDeckMux {
    /// C1 all the way left
    /// B1 all the way left
    /// Keyboard left
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
    /// Not a module position, implementation detail
    _TopRowNotConnected,
    /// Not a module position, implementation detail
    _Hubboard = 7,
}

#[repr(u8)]
#[derive(Debug)]
pub enum InputModuleType {
    Short,
    Reserved1,
    Reserved2,
    Reserved3,
    Reserved4,
    Reserved5,
    Reserved6,
    Reserved7,
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
            6 => Self::Reserved6,
            7 => Self::Reserved7,
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

#[derive(Debug)]
pub enum InputDeckState {
    Off,
    Disconnected,
    TurningOn,
    On,
    ForceOff,
    ForceOn,
    /// Input deck will follow power sequence, no present check
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

pub struct InputDeckStatus {
    pub state: InputDeckState,
    pub touchpad_present: bool,
    pub top_row: TopRowPositions,
}

impl From<EcResponseDeckState> for InputDeckStatus {
    fn from(item: EcResponseDeckState) -> Self {
        InputDeckStatus {
            state: InputDeckState::from(item.deck_state),
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
//impl TryFrom<EcResponseDeckState> for InputDeckStatus {
//    type Error = ();
//
//    fn try_from(value: EcResponseDeckState) -> Result<Self, Self::Error> {
//        if value % 2 == 0 {
//            Ok(EvenNumber(value))
//        } else {
//            Err(())
//        }
//    }
//}

pub struct TopRowPositions {
    /// C1 all the way left
    /// B1 all the way left
    /// Keyboard left
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

//pub enum TopRowShapes {
//    ThinThinKeyboard,
//    ThinKeyboardThin,
//    KeyboardThinThin,
//}
//impl From<TopRowPositions> for TopRowShapes {
//    fn from(item: EcResponseDeckState) -> Self {
//        TopRowShapes
//    }
//}
