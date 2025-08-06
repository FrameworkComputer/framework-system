use alloc::fmt;

#[cfg(not(feature = "uefi"))]
use guid_create::GUID;
#[cfg(feature = "uefi")]
use guid_create_no_std::GUID;

// Until https://github.com/kurtlawrence/guid-create/pull/20 is merged

impl From<CGuid> for GUID {
    fn from(item: CGuid) -> Self {
        GUID::build_from_components(item.a, item.b, item.c, &item.d)
    }
}

impl From<GUID> for CGuid {
    fn from(item: GUID) -> Self {
        CGuid {
            a: item.data1(),
            b: item.data2(),
            c: item.data3(),
            d: item.data4(),
        }
    }
}

impl fmt::Display for CGuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:08X}-{:04X}-{:04X}-{:04X}-{:08X}{:04X}",
            self.a,
            self.b,
            self.c,
            u16::from_be_bytes(self.d[0..2].try_into().unwrap()),
            u32::from_be_bytes(self.d[2..6].try_into().unwrap()),
            u16::from_be_bytes(self.d[6..8].try_into().unwrap()),
        )
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Hash)]
#[repr(C)]
pub struct CGuid {
    /// The low field of the timestamp.
    a: u32,
    /// The middle field of the timestamp.
    b: u16,
    /// The high field of the timestamp multiplexed with the version number.
    c: u16,
    /// Contains, in this order:
    /// - The high field of the clock sequence multiplexed with the variant.
    /// - The low field of the clock sequence.
    /// - The spatially unique node identifier.
    d: [u8; 8],
}
