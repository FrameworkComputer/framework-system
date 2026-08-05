//! Get the Intel CNVi Wi-Fi SAR power tables from UEFI variables.
//!
//! Currently only implemented on Linux (needs root) and UEFI.
//!
//! SAR (Specific Absorption Rate) limits are the maximum TX power the Wi-Fi
//! module may use per frequency sub-band. The BIOS hands them to the driver in
//! UEFI variables. WRDS holds SAR profile 1, EWRD the remaining profiles.
//!
//! The layouts are the same as in the Linux iwlwifi driver, see
//! `drivers/net/wireless/intel/iwlwifi/fw/uefi.h` (struct uefi_cnv_var_wrds and
//! uefi_cnv_var_ewrd).

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use std::prelude::v1::*;

#[cfg(all(not(feature = "uefi"), target_os = "linux"))]
use std::fs;

#[cfg(feature = "uefi")]
use uefi::runtime::{self, VariableVendor};
#[cfg(feature = "uefi")]
use uefi::{guid, CStr16};

/// Vendor GUID of the Intel CNVi Wi-Fi UEFI variables
pub const CNVI_WIFI_GUID: &str = "92daaf2f-c02b-455b-b2ec-f5a3594f4aea";
#[cfg(feature = "uefi")]
const CNVI_WIFI_VENDOR: VariableVendor =
    VariableVendor(guid!("92daaf2f-c02b-455b-b2ec-f5a3594f4aea"));

const WRDS_VAR: &str = "UefiCnvWlanWRDS";
const EWRD_VAR: &str = "UefiCnvWlanEWRD";

/// UEFI_SAR_MAX_CHAINS_PER_PROFILE
const CHAINS: usize = 4;
const CHAIN_NAMES: [&str; CHAINS] = ["ChainA", "ChainB", "CdbChainA", "CdbChainB"];
/// BIOS_SAR_MAX_PROFILE_NUM
const MAX_PROFILES: usize = 4;
/// IWL_SAR_ENABLE_MSK
const SAR_ENABLE: u32 = 1;

/// Vendor names and frequency ranges of the revision 2 sub-bands.
/// Revision 3 adds a 12th sub-band that neither the driver nor the vendor names.
const SUB_BANDS: [(&str, &str); 11] = [
    ("2.4G", "2400"),
    ("5G2/3", "5180-5320"),
    ("5G4", "5340-5480"),
    ("5G6", "5500-5720"),
    ("5G8/9", "5745-5885"),
    ("6G1", "5955-6175"),
    ("6G3", "6195-6415"),
    ("6G5", "6435-6515"),
    ("6G6", "6535-6695"),
    ("6G8", "6715-6855"),
    ("7G0", "6875-7115"),
];

/// UEFI_SAR_SUB_BANDS_NUM_REV2 and _REV3
fn sub_bands(revision: u8) -> Option<usize> {
    match revision {
        2 => Some(11),
        3 => Some(12),
        _ => None,
    }
}

/// Names and frequency ranges of the sub-bands of a table with `n` of them
fn sub_band_names(n: usize) -> impl Iterator<Item = (&'static str, &'static str)> {
    (0..n).map(|i| *SUB_BANDS.get(i).unwrap_or(&("?", "?")))
}

#[derive(Debug)]
pub struct SarTable {
    pub revision: u8,
    /// Bit 0 tells the driver whether to apply SAR at all
    pub mode: u32,
    /// How many of the profiles the BIOS actually filled in. Only in EWRD.
    pub num_profiles: Option<u32>,
    /// Number of the first profile in `profiles`. 1 for WRDS, 2 for EWRD.
    pub first_profile: usize,
    /// Power limits, indexed by profile, chain and sub-band, in 1/8 dBm
    pub profiles: Vec<Vec<Vec<u8>>>,
}

impl SarTable {
    fn is_used(&self, profile: usize) -> bool {
        match self.num_profiles {
            // WRDS' single profile is always in use
            None => true,
            Some(num) => (profile - self.first_profile) < num as usize,
        }
    }
}

/// Split the power limits of `profiles` profiles into profiles, chains and sub-bands
fn parse_profiles(data: &[u8], profiles: usize, sub_bands: usize) -> Vec<Vec<Vec<u8>>> {
    let per_profile = CHAINS * sub_bands;
    (0..profiles)
        .map(|p| {
            (0..CHAINS)
                .map(|c| {
                    let start = p * per_profile + c * sub_bands;
                    data[start..start + sub_bands].to_vec()
                })
                .collect()
        })
        .collect()
}

/// Parse WRDS, which holds SAR profile 1
///
/// struct uefi_cnv_var_wrds: u8 revision, u32 mode, then one profile
pub fn parse_wrds(data: &[u8]) -> Option<SarTable> {
    let revision = *data.first()?;
    let sub_bands = sub_bands(revision).or_else(|| {
        error!("WRDS: Unsupported revision {}", revision);
        None
    })?;
    let expected = 5 + CHAINS * sub_bands;
    if data.len() != expected {
        error!(
            "WRDS: Revision {} should be {} bytes, got {}",
            revision,
            expected,
            data.len()
        );
        return None;
    }

    Some(SarTable {
        revision,
        mode: u32::from_le_bytes(data[1..5].try_into().unwrap()),
        num_profiles: None,
        first_profile: 1,
        profiles: parse_profiles(&data[5..], 1, sub_bands),
    })
}

/// Parse EWRD, which holds SAR profiles 2 and up
///
/// struct uefi_cnv_var_ewrd: u8 revision, u32 mode, u32 num_profiles,
/// then MAX_PROFILES-1 profiles
pub fn parse_ewrd(data: &[u8]) -> Option<SarTable> {
    let revision = *data.first()?;
    let sub_bands = sub_bands(revision).or_else(|| {
        error!("EWRD: Unsupported revision {}", revision);
        None
    })?;
    let profiles = MAX_PROFILES - 1;
    let expected = 9 + CHAINS * sub_bands * profiles;
    if data.len() != expected {
        error!(
            "EWRD: Revision {} should be {} bytes, got {}",
            revision,
            expected,
            data.len()
        );
        return None;
    }

    let num_profiles = u32::from_le_bytes(data[5..9].try_into().unwrap());
    if num_profiles as usize > profiles {
        error!("EWRD: Invalid number of profiles: {}", num_profiles);
        return None;
    }

    Some(SarTable {
        revision,
        mode: u32::from_le_bytes(data[1..5].try_into().unwrap()),
        num_profiles: Some(num_profiles),
        first_profile: 2,
        profiles: parse_profiles(&data[9..], profiles, sub_bands),
    })
}

/// Format a power limit of 1/8 dBm units as dBm
fn dbm(limit: u8) -> String {
    format!("{:.1}", f32::from(limit) / 8.0)
}

pub fn print_sar_table(name: &str, table: &SarTable) {
    let sub_bands = table.profiles[0][0].len();
    println!("{}", name);
    println!(
        "  Revision:             {} ({} chains, {} sub-bands)",
        table.revision, CHAINS, sub_bands
    );
    println!(
        "  Mode:                 0x{:08X} (SAR {})",
        table.mode,
        if table.mode & SAR_ENABLE != 0 {
            "enabled"
        } else {
            "disabled"
        }
    );
    if let Some(num_profiles) = table.num_profiles {
        println!("  Profiles in use:      {}", num_profiles);
    }

    for (i, chains) in table.profiles.iter().enumerate() {
        let profile = table.first_profile + i;
        let unused = if table.is_used(profile) {
            ""
        } else {
            " (unused)"
        };
        // Most systems only populate ChainA and ChainB
        let used: Vec<usize> = (0..CHAINS)
            .filter(|c| chains[*c].iter().any(|limit| *limit != 0))
            .collect();
        if used.is_empty() {
            println!("  Profile {}{}: All zero", profile, unused);
            continue;
        }

        println!("  Profile {}{}", profile, unused);
        print!("    {:<7}  {:<11}", "Subband", "Range (MHz)");
        for c in &used {
            print!("  {:<13}", CHAIN_NAMES[*c]);
        }
        println!();
        for (s, (name, range)) in sub_band_names(sub_bands).enumerate() {
            print!("    {:<7}  {:<11}", name, range);
            for c in &used {
                print!("  0x{:02X} {:>4} dBm", chains[*c][s], dbm(chains[*c][s]));
            }
            println!();
        }
        let zero: Vec<&str> = (0..CHAINS)
            .filter(|c| !used.contains(c))
            .map(|c| CHAIN_NAMES[c])
            .collect();
        if !zero.is_empty() {
            println!("    All zero: {}", zero.join(", "));
        }
    }
}

pub fn print_wifi_sar() -> Option<()> {
    let mut found = false;

    if let Some(data) = get_variable(WRDS_VAR) {
        if let Some(table) = parse_wrds(&data) {
            print_sar_table("WRDS - Wi-Fi SAR limits", &table);
            found = true;
        }
    }
    if let Some(data) = get_variable(EWRD_VAR) {
        if let Some(table) = parse_ewrd(&data) {
            print_sar_table("EWRD - Additional Wi-Fi SAR limits", &table);
            found = true;
        }
    }

    if found {
        Some(())
    } else {
        None
    }
}

/// Read a UEFI variable of the Intel CNVi Wi-Fi vendor GUID
#[cfg(all(not(feature = "uefi"), target_os = "linux"))]
fn get_variable(name: &str) -> Option<Vec<u8>> {
    let path = format!("/sys/firmware/efi/efivars/{}-{}", name, CNVI_WIFI_GUID);
    let data = fs::read(&path)
        .map_err(|err| {
            error!("Failed to read {}: {}", path, err);
            info!("Make sure you're root to access UEFI variables from sysfs on Linux");
        })
        .ok()?;
    // In sysfs the payload is prefixed by the 32bit variable attributes
    if data.len() < 4 {
        error!("{} is too short: {} bytes", path, data.len());
        return None;
    }
    Some(data[4..].to_vec())
}

#[cfg(feature = "uefi")]
fn get_variable(name: &str) -> Option<Vec<u8>> {
    let mut buf = [0; 32];
    let name = CStr16::from_str_with_buf(name, &mut buf)
        .map_err(|err| error!("Invalid variable name {}: {:?}", name, err))
        .ok()?;
    runtime::get_variable_boxed(name, &CNVI_WIFI_VENDOR)
        .map_err(|err| error!("Failed to read UEFI variable {}: {:?}", name, err))
        .ok()
        .map(|(data, _attributes)| data.to_vec())
}

#[cfg(all(not(feature = "uefi"), not(target_os = "linux")))]
fn get_variable(_name: &str) -> Option<Vec<u8>> {
    error!("Reading UEFI variables is not implemented on this OS");
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WRDS and EWRD as dumped from a Framework Laptop 13 (Intel Core Ultra 1)
    const WRDS: &[u8] = &[
        0x02, 0x01, 0x00, 0x00, 0x00, 0x80, 0x70, 0x70, 0x74, 0x6c, 0x6c, 0x6c, 0x6c, 0x6c, 0x6c,
        0x6c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn parse_wrds_table() {
        // ChainB holds the same limits as ChainA, CdbChain{A,B} are unused
        let mut data = WRDS.to_vec();
        data[16..27].copy_from_slice(&WRDS[5..16]);

        let table = parse_wrds(&data).unwrap();
        assert_eq!(table.revision, 2);
        assert_eq!(table.mode, SAR_ENABLE);
        assert_eq!(table.num_profiles, None);
        assert_eq!(table.first_profile, 1);
        assert_eq!(table.profiles.len(), 1);
        // 0x80 = 16 dBm in 1/8 dBm units
        assert_eq!(table.profiles[0][0][0], 0x80);
        assert_eq!(dbm(table.profiles[0][0][0]), "16.0");
        assert_eq!(dbm(table.profiles[0][1][10]), "13.5");
        assert_eq!(table.profiles[0][2], [0; 11]);
        assert_eq!(table.profiles[0][3], [0; 11]);
    }

    #[test]
    fn parse_wrds_rejects_bad_table() {
        // Unsupported revision
        let mut data = WRDS.to_vec();
        data[0] = 1;
        assert!(parse_wrds(&data).is_none());

        // Truncated
        assert!(parse_wrds(&WRDS[..WRDS.len() - 1]).is_none());
        assert!(parse_wrds(&[]).is_none());
    }

    #[test]
    fn parse_ewrd_table() {
        // Revision 2, SAR enabled, one extra profile
        let mut data = vec![0x02, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        data.resize(9 + CHAINS * 11 * (MAX_PROFILES - 1), 0);
        data[9] = 0xa8;

        let table = parse_ewrd(&data).unwrap();
        assert_eq!(table.revision, 2);
        assert_eq!(table.num_profiles, Some(1));
        assert_eq!(table.first_profile, 2);
        assert_eq!(table.profiles.len(), MAX_PROFILES - 1);
        assert_eq!(dbm(table.profiles[0][0][0]), "21.0");
        // Only profile 2 is in use, 3 and 4 are beyond num_profiles
        assert!(table.is_used(2));
        assert!(!table.is_used(3));
        assert!(!table.is_used(4));

        // More profiles than the firmware can hold
        data[5] = 0x04;
        assert!(parse_ewrd(&data).is_none());
    }
}
