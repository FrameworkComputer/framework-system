//! Fuzz parsing of STM32 keyboard firmware HEX files (--flash-stm32-kbd)
//! and planning the flash writes for them
#![no_main]

use framework_lib::stm32_kbd;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(img) = stm32_kbd::HexImage::parse(text) else {
        return;
    };
    let _ = img.end();
    if let Some(version) = img.find_version() {
        let _ = stm32_kbd::format_version(version);
    }
    for protect in [false, true] {
        stm32_kbd::fuzz_plan(&img, protect);
    }
});
