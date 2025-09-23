use alloc::vec::Vec;
use num_traits::FromPrimitive;

use crate::chromium_ec::command::EcRequestRaw;
use crate::chromium_ec::commands::{EcRequestGetGpuPcie, GpuVendor};
use crate::chromium_ec::i2c_passthrough::*;
use crate::chromium_ec::{CrosEc, EcResult};
use crate::os_specific;

pub fn get_version(ec: &CrosEc) -> EcResult<Option<Vec<u8>>> {
    let res = EcRequestGetGpuPcie {}.send_command(ec)?;
    let vendor: Option<GpuVendor> = FromPrimitive::from_u8(res.gpu_vendor);
    if vendor != Some(GpuVendor::NvidiaGn22) {
        debug!("No compatible retimer present");
        return Ok(None);
    };

    // I2C Port on the EC
    let i2c_port = 5;
    // 8-bit I2C address of the retimer
    // EC passthrough needs 7-bit, so shift one over before sending to EC
    let i2c_addr = 0x10;

    // Check safe mode
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x00, 0x01)?;
    if i2c_response.data[0] == 0 {
        // Safe mode not enabled, enable it
        i2c_write(ec, i2c_port, i2c_addr >> 1, 0x00, &[0x01])?;
    }

    // Wake up from low power mode
    for _ in 0..3 {
        let i2c_response = i2c_read(ec, i2c_port, (i2c_addr + 2) >> 1, 0x70, 0x01)?;
        if i2c_response.data[0] != 0 {
            i2c_write(ec, i2c_port, (i2c_addr + 2) >> 1, 0x70, &[0x00])?;
            os_specific::sleep(50_000);
        }
    }

    // Read version
    let i2c_response = i2c_read(ec, i2c_port, (i2c_addr + 18) >> 1, 0x01, 0x04)?;

    Ok(Some(i2c_response.data))
}
