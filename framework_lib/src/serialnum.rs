//! Decode Framework part serial numbers
//!
//! Format "A" (18 ASCII characters). Except for week and day all fields use
//! the Crockford base32 alphabet: 0-9 and A-Z without I, L, O, U.
//!
//! ```text
//! FR A NBD CP A 1 4 05 5 0001
//! |  | |   |  | | | |  | +- part:    4 base32 characters, unit number
//! |  | |   |  | | | |  +--- day:     ISO 8601 weekday, 1 (Monday) - 7 (Sunday)
//! |  | |   |  | | | +------ week:    ISO 8601 week, 01 - 53
//! |  | |   |  | | +-------- year:    2020 + base32 value (2020 - 2051)
//! |  | |   |  | +---------- cfg1:    product specific config
//! |  | |   |  +------------ cfg0:    usually development phase, see [`Cfg0`]
//! |  | |   +--------------- oem:     2 base32 characters
//! |  | +------------------- product: 3 base32 characters
//! |  +--------------------- format:  always A
//! +------------------------ brand:   always FR
//! ```

use alloc::format;
use alloc::string::{String, ToString};
use core::fmt;
use core::str::FromStr;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

const SERIAL_LEN: usize = 18;
/// Year that is encoded as 0
const BASE_YEAR: u16 = 2020;

#[derive(Debug)]
pub struct FrameworkSerial {
    /// Three character string
    pub product: String,
    /// Two character string
    pub oem: String,
    /// First config character, usually the development phase. See [`Self::phase`]
    pub cfg0: char,
    /// Defines config of that specific product
    pub cfg1: char,
    pub year: u16,
    /// ISO 8601 week of the year (1-53)
    pub week: u8,
    pub day: WeekDay,
    /// Four letter/digit string
    pub part: String,
}

#[repr(u8)]
#[derive(Debug, PartialEq, FromPrimitive, Clone, Copy)]
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
    MassProduction = 0x0A,
    MassProductionB = 0x0B,
    MassProductionC = 0x0C,
    MassProductionD = 0x0D,
    MassProductionE = 0x0E,
    MassProductionF = 0x0F,
}

#[repr(u8)]
#[derive(Debug, PartialEq, FromPrimitive, Clone, Copy)]
pub enum WeekDay {
    Monday = 1,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

/// Crockford base32 alphabet, index is the value
const BASE32: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

fn base32_value(c: char) -> Option<u8> {
    BASE32.find(c).map(|i| i as u8)
}

fn is_base32(c: char) -> bool {
    base32_value(c).is_some()
}

/// Take `len` characters from the front of `s` and check each against `valid`
fn field<'a>(
    s: &mut &'a str,
    len: usize,
    name: &str,
    valid: fn(char) -> bool,
) -> Result<&'a str, String> {
    let (value, rest) = s.split_at(len);
    *s = rest;
    if value.chars().all(valid) {
        Ok(value)
    } else {
        Err(format!("Invalid {} '{}'", name, value))
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_year(year: i32) -> i32 {
    if is_leap_year(year) {
        366
    } else {
        365
    }
}

/// ISO weekday (1 = Monday, 7 = Sunday) of a calendar date
fn iso_weekday(year: i32, month: u32, day: u32) -> u32 {
    // Sakamoto's method, returns 0 = Sunday
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    let dow = (y + y / 4 - y / 100 + y / 400 + T[month as usize - 1] + day as i32) % 7;
    if dow == 0 {
        7
    } else {
        dow as u32
    }
}

/// Number of ISO weeks in a year (52 or 53)
fn iso_weeks_in_year(year: i32) -> u8 {
    let jan1 = iso_weekday(year, 1, 1);
    if jan1 == 4 || (jan1 == 3 && is_leap_year(year)) {
        53
    } else {
        52
    }
}

impl FrameworkSerial {
    /// Development phase, if CFG0 is one of the known phases (0-F)
    pub fn phase(&self) -> Option<Cfg0> {
        self.cfg0.to_digit(16).and_then(|d| Cfg0::from_u8(d as u8))
    }

    /// Calendar date (year, month, day) of the ISO week date in the serial
    pub fn date(&self) -> (i32, u32, u32) {
        let mut year = self.year as i32;
        // Ordinal day of the year, see ISO 8601 week date
        let mut ordinal =
            self.week as i32 * 7 + self.day as i32 - (iso_weekday(year, 1, 4) as i32 + 3);
        if ordinal < 1 {
            year -= 1;
            ordinal += days_in_year(year);
        } else if ordinal > days_in_year(year) {
            ordinal -= days_in_year(year);
            year += 1;
        }

        let feb = if is_leap_year(year) { 29 } else { 28 };
        let month_days = [31, feb, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
        let mut month = 1;
        for days in month_days {
            if ordinal <= days {
                break;
            }
            ordinal -= days;
            month += 1;
        }
        (year, month, ordinal as u32)
    }
}

impl fmt::Display for FrameworkSerial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (year, month, day) = self.date();
        if let Some(phase) = self.phase() {
            write!(
                f,
                "{} (Config {}) by {}, {:>4} Phase",
                self.product,
                self.cfg1,
                self.oem,
                format!("{:?}", phase),
            )?;
        } else {
            write!(
                f,
                "{} (Config {}{}) by {}",
                self.product, self.cfg0, self.cfg1, self.oem,
            )?;
        }
        write!(
            f,
            " ({:?} {}-{:02}-{:02}, Week {})",
            self.day, year, month, day, self.week,
        )
    }
}

impl FromStr for FrameworkSerial {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.len() != SERIAL_LEN || !s.is_ascii() {
            return Err(format!("Invalid serial length or encoding '{}'", s));
        }
        let mut rest = s;

        field(&mut rest, 3, "brand/format", |_| true).and_then(|prefix| {
            if prefix == "FRA" {
                Ok(())
            } else {
                Err(format!("Invalid brand/format '{}'", prefix))
            }
        })?;
        let product = field(&mut rest, 3, "product", is_base32)?;
        let oem = field(&mut rest, 2, "OEM", is_base32)?;
        let cfg0 = field(&mut rest, 1, "CFG0", is_base32)?;
        let cfg1 = field(&mut rest, 1, "CFG1", is_base32)?;
        let year = field(&mut rest, 1, "year", is_base32)?;
        let week = field(&mut rest, 2, "week", |c| c.is_ascii_digit())?;
        let day = field(&mut rest, 1, "day", |c| c.is_ascii_digit())?;
        let part = field(&mut rest, 4, "part", is_base32)?;

        // Single character fields are exactly one ASCII character
        let char_of = |s: &str| s.as_bytes()[0] as char;
        // Validated as base32 above, cannot fail
        let year = BASE_YEAR + base32_value(char_of(year)).unwrap_or(0) as u16;
        // Validated as ASCII digits above, parsing cannot fail
        let week = week.parse::<u8>().map_err(|e| e.to_string())?;
        if week == 0 || week > iso_weeks_in_year(year as i32) {
            return Err(format!("Invalid week {} in {}", week, year));
        }
        let day = day
            .parse::<u8>()
            .ok()
            .and_then(WeekDay::from_u8)
            .ok_or_else(|| format!("Invalid day '{}'", day))?;

        Ok(FrameworkSerial {
            product: product.to_string(),
            oem: oem.to_string(),
            cfg0: char_of(cfg0),
            cfg1: char_of(cfg1),
            year,
            week,
            day,
            part: part.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base32_year() {
        let serial = FrameworkSerial::from_str("FRANBDCP71X4040001").unwrap();
        assert_eq!(serial.year, 2049);
        assert_eq!(serial.date(), (2049, 10, 7));
        assert_eq!(
            FrameworkSerial::from_str("FRANBDCPA1A0110001")
                .unwrap()
                .year,
            2030
        );
        assert_eq!(
            FrameworkSerial::from_str("FRANBDCPA1Z5270001")
                .unwrap()
                .year,
            2051
        );
    }

    #[test]
    fn hex_cfg0() {
        let serial = FrameworkSerial::from_str("FRANBDCPA140550001").unwrap();
        assert_eq!(serial.cfg0, 'A');
        assert_eq!(serial.phase(), Some(Cfg0::MassProduction));
    }

    #[test]
    fn non_hex_cfg0() {
        // Spec allows any base32 config, phase is unknown
        let serial = FrameworkSerial::from_str("FRANBDCPG140550001").unwrap();
        assert_eq!(serial.cfg0, 'G');
        assert_eq!(serial.phase(), None);
        assert_eq!(
            serial.to_string(),
            "NBD (Config G1) by CP (Friday 2024-02-02, Week 5)"
        );
    }

    #[test]
    fn digit_product() {
        let serial = FrameworkSerial::from_str("FRAN2DCPA140550001").unwrap();
        assert_eq!(serial.product, "N2D");
    }

    /// Examples from the serialization spec
    #[test]
    fn spec_examples() {
        let serial = FrameworkSerial::from_str("FRANBZCP0002320001").unwrap();
        assert_eq!(serial.product, "NBZ");
        assert_eq!(serial.oem, "CP");
        assert_eq!(serial.part, "0001");
        // The spec's verify example says "02 June 2020"
        assert_eq!(serial.date(), (2020, 6, 2));
        assert_eq!(
            serial.to_string(),
            "NBZ (Config 0) by CP,  SKU Phase (Tuesday 2020-06-02, Week 23)"
        );

        let serial = FrameworkSerial::from_str("FRAAAACP0001220001").unwrap();
        assert_eq!(serial.date(), (2020, 3, 17));
    }

    #[test]
    fn letter_cfg1() {
        let serial = FrameworkSerial::from_str("FRANBDCP7H40550001").unwrap();
        assert_eq!(serial.cfg1, 'H');
    }

    #[test]
    fn numeric_oem() {
        let serial = FrameworkSerial::from_str("FRANBD00A140550001").unwrap();
        assert_eq!(serial.oem, "00");
    }

    #[test]
    fn part() {
        let serial = FrameworkSerial::from_str("FRANBDCPA14055AB12").unwrap();
        assert_eq!(serial.part, "AB12");
    }

    #[test]
    fn trailing_whitespace() {
        assert!(FrameworkSerial::from_str("FRANBDCPA14055AB12   ").is_ok());
    }

    #[test]
    fn invalid() {
        for s in [
            "",
            "FRANBDCPA14055AB1",   // too short
            "FRANBDCPA14055AB123", // too long
            "FRBNBDCPA14055AB12",  // wrong format
            "FRAnbdCPA14055AB12",  // lowercase product
            "FRAINDCPA14055AB12",  // I is not base32
            "FRANBDCOA14055AB12",  // O is not base32
            "FRANBDCPL14055AB12",  // L is not base32
            "FRANBDCPA1U055AB12",  // U is not base32
            "FRANBDCPA14055AB1I",  // I is not base32
            "FRANBDCPA14005AB12",  // week 0
            "FRANBDCPA14535AB12",  // 2024 has only 52 weeks
            "FRANBDCPA14050AB12",  // day 0
            "FRANBDCPA14058AB12",  // day 8
            "FRANBDCPA1405öAB1",   // non-ASCII
        ] {
            assert!(FrameworkSerial::from_str(s).is_err(), "{}", s);
        }
    }

    #[test]
    fn date() {
        let date = |s| FrameworkSerial::from_str(s).unwrap().date();
        // Monday, Week 20, 2024
        assert_eq!(date("FRANBDCPA14201AB12"), (2024, 5, 13));
        // Wednesday, Week 44, 2023
        assert_eq!(date("FRANBDCPA13443AB12"), (2023, 11, 1));
        // Monday of week 1 can be in the previous year
        assert_eq!(date("FRANBDCPA15011AB12"), (2024, 12, 30));
        // 2020 has 53 weeks, Sunday of week 53 is in the next year
        assert_eq!(date("FRANBDCPA10537AB12"), (2021, 1, 3));
        // Leap day
        assert_eq!(date("FRANBDCPA14094AB12"), (2024, 2, 29));
    }

    #[test]
    fn display() {
        let serial = FrameworkSerial::from_str("FRANBDCPA14201AB12").unwrap();
        assert_eq!(
            serial.to_string(),
            "NBD (Config 1) by CP, MassProduction Phase (Monday 2024-05-13, Week 20)"
        );
    }
}
