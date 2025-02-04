use alloc::string::{String, ToString};
use core::str::FromStr;

#[derive(Debug)]
pub struct FrameworkSerial {
    // brand:  Always FR for Framework
    // format: Always A
    /// Three letter string
    pub product: String,
    /// Two letter string
    pub oem: String,
    /// Development state
    pub cfg0: Cfg0,
    /// Defines config of that specific product
    pub cfg1: char,
    pub year: u16,
    pub week: u8,
    pub day: WeekDay,
    /// Four letter/digit string
    pub part: String,
}

#[derive(Debug)]
pub enum Cfg0 {
    SKU = 0x00,
    Poc1 = 0x01,
    Proto1 = 0x02,
    Proto2 = 0x03,
    Evt1 = 0x04,
    Evt2 = 0x05,
    Reserved = 0x06,
    Dvt1 = 0x07,
    Dvt2 = 0x08,
    Pvt = 0x09,
    Mp = 0x0A,
}

#[derive(Debug)]
pub enum WeekDay {
    Monday = 1,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl FromStr for FrameworkSerial {
    type Err = String;

    // TODO: !!! PROPER ERROR HANDLING !!!
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pattern =
            r"FRA([A-Z]{3})([A-Z]{2})([0-9A-F])([0-9A-F])([0-9A-Z])([0-9]{2})([0-7])([0-9A-Z]{4})";
        let re = regex::Regex::new(pattern).unwrap();

        let caps = re.captures(s).ok_or("Invalid Serial".to_string())?;

        let cfg0 = match caps.get(3).unwrap().as_str().chars().next().unwrap() {
            '0' => Cfg0::SKU,
            '1' => Cfg0::Poc1,
            '2' => Cfg0::Proto1,
            '3' => Cfg0::Proto2,
            '4' => Cfg0::Evt1,
            '5' => Cfg0::Evt2,
            '6' => Cfg0::Reserved,
            '7' => Cfg0::Dvt1,
            '8' => Cfg0::Dvt2,
            '9' => Cfg0::Pvt,
            'A' => Cfg0::Mp,
            _ => return Err("Invalid CFG0".to_string()),
        };
        let cfg1 = caps.get(4).unwrap().as_str().chars().next().unwrap();
        let year = str::parse::<u16>(caps.get(5).unwrap().as_str()).unwrap();
        let year = 2020 + year;
        let week = str::parse::<u8>(caps.get(6).unwrap().as_str()).unwrap();
        // TODO: Decode into date
        let day = match str::parse::<u8>(caps.get(7).unwrap().as_str()).unwrap() {
            1 => WeekDay::Monday,
            2 => WeekDay::Tuesday,
            3 => WeekDay::Wednesday,
            4 => WeekDay::Thursday,
            5 => WeekDay::Friday,
            6 => WeekDay::Saturday,
            7 => WeekDay::Sunday,
            _ => return Err("Invalid Day".to_string()),
        };

        Ok(FrameworkSerial {
            product: caps.get(1).unwrap().as_str().to_string(),
            oem: caps.get(2).unwrap().as_str().to_string(),
            cfg0,
            cfg1,
            year,
            week,
            day,
            part: caps.get(2).unwrap().as_str().to_string(),
        })
    }
}
