# Advanced debugging

## PD

### Check PD state

Example on Framework 13 AMD Ryzen AI 300

```
> sudo framework_tool.exe --pd-info
Right / Ports 01
  Silicon ID:     0x3580
  Mode:           MainFw
  Flash Row Size: 256 B
  Ports Enabled:  0, 1
  Bootloader Version:   Base: 3.6.0.009,  App: 0.0.01
  FW1 (Backup) Version: Base: 3.7.0.197,  App: 0.0.0B
  FW2 (Main)   Version: Base: 3.7.0.197,  App: 0.0.0B
Left / Ports 23
  Silicon ID:     0x3580
  Mode:           MainFw
  Flash Row Size: 256 B
  Ports Enabled:  0, 1
  Bootloader Version:   Base: 3.6.0.009,  App: 0.0.01
  FW1 (Backup) Version: Base: 3.7.0.197,  App: 0.0.0B
  FW2 (Main)   Version: Base: 3.7.0.197,  App: 0.0.0B
```

### Disable/enable/reset PD

```
# Disable all ports on PD 0
> sudo framework_tool --pd-disable 0

# Reset PD 0 (enables all ports again)
> sudo framework_tool --pd-reset 0

# Or enable all ports on PD 0 without resetting it
> sudo framework_tool --pd-enable 0
```

### Check EFI Resource Table

On Framework Desktop:

```
> sudo framework_tool --esrt
ESRT Table
  ResourceCount:        1
  ResourceCountMax:     1
  ResourceVersion:      1
ESRT Entry 0
  GUID:                 EB68DBAE-3AEF-5077-92AE-9016D1F0C856
  GUID:                 DesktopAmdAi300Bios
  Type:                 SystemFirmware
  Version:              0x204 (516)
  Min FW Version:       0x100 (256)
  Capsule Flags:        0x0
  Last Attempt Version: 0x108 (264)
  Last Attempt Status:  Success
```

## Flashing EC firmware

**IMPORTANT** Flashing EC firmware yourself is not recommended. It may render
your hardware unbootable. Please update your firmware using the official BIOS
update methods (Windows .exe, LVFS/FWUPD, EFI updater)!

This command has not been thoroughly tested on all Framework Computer systems

```
# Simulate flashing RW (to see which blocks are updated)
> framework_tool --flash-rw-ec ec.bin --dry-run

# Actually flash RW
> framework_tool --flash-rw-ec ec.bin

# Boot into EC RW firmware (will crash your OS and reboot immediately)
# EC will boot back into RO if the system turned off for 30s
> framework_tool --reboot-ec jump-rw
```

## Flashing Expansion Bay EEPROM (Framework 16)

This will render your dGPU unsuable if you flash the wrong file!
It's intended for advanced users who build their own expansion bay module.
The I2C address of the EEPROM is hardcoded to 0x50.

```
# Dump current descriptor (e.g. for backup)
> framework_tool --dump-gpu-descriptor-file foo.bin
Dumping to foo.bin
Wrote 153 bytes to foo.bin

# Update just the serial number
> framework_tool --flash_gpu_descriptor GPU FRAKMQCP41500ASSY1
> framework_tool --flash_gpu_descriptor 13 FRAKMQCP41500ASSY1
> framework_tool --flash_gpu_descriptor 0x0D FRAKMQCP41500ASSY1

# Update everything from a file
> framework_tool --flash-gpu-descriptor-file pcie_4x2.bin
```
