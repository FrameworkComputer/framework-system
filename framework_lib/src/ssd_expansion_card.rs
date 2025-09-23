/// Firmware Versions
/// | Flash Die    | Firmware Version |
/// |--------------|------------------|
/// | Hynix V6     | UHFM00.x         |
/// | Micron N28   | UHFM10.x         |
/// | Micron N48   | UHFM30.x         |
/// | Kioxia BiSC6 | UHFM90.x         |
///
/// On Linux: sudo smartctl -ji /dev/sda | jq -r .firmware_version
///   Need to install smartmontools
/// On Windows
///   winget
///     winget install --id=smartmontools.smartmontools -e
///     winget install --id=jqlang.jq  -e
