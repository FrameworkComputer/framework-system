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
    info!("GPU vendor: {:?}", vendor);
    if vendor != Some(GpuVendor::NvidiaGn22) {
        info!("No compatible retimer present (vendor mismatch)");
        return Ok(None);
    };
    info!("NVIDIA GPU detected, checking retimer...");

    // I2C Port on the EC
    let i2c_port = 5;
    // 8-bit I2C address of the retimer
    // EC passthrough needs 7-bit, so shift one over before sending to EC
    let i2c_addr = 0x10;

    // Check safe mode
    info!(
        "Reading retimer at I2C port {} addr 0x{:02X}",
        i2c_port,
        i2c_addr >> 1
    );
    let i2c_response = i2c_read(ec, i2c_port, i2c_addr >> 1, 0x00, 0x01)?;
    info!(
        "I2C response: status=0x{:02X}, data_len={}",
        i2c_response.i2c_status,
        i2c_response.data.len()
    );
    if i2c_response.i2c_status == 0x01 {
        // NAK
        warn!("Unable to communicate with dGPU Retimer. Try to force it on by plugging a cable into the dGPU");
    }
    let Some(&safe_mode) = i2c_response.data.first() else {
        info!("Failed to read retimer safe mode status (empty response)");
        return Ok(None);
    };
    info!("Retimer safe mode status: {}", safe_mode);
    if safe_mode == 0 {
        // Safe mode not enabled, enable it
        i2c_write(ec, i2c_port, i2c_addr >> 1, 0x00, &[0x01])?;
    }

    // Wake up from low power mode
    for _ in 0..3 {
        let i2c_response = i2c_read(ec, i2c_port, (i2c_addr + 2) >> 1, 0x70, 0x01)?;
        if i2c_response.data.first() == Some(&0) {
            continue;
        }
        i2c_write(ec, i2c_port, (i2c_addr + 2) >> 1, 0x70, &[0x00])?;
        os_specific::sleep(50_000);
    }

    // Read version
    let i2c_response = i2c_read(ec, i2c_port, (i2c_addr + 18) >> 1, 0x01, 0x04)?;

    Ok(Some(i2c_response.data))
}
