# Advanced debugging

## PD

### Check PD state

Example on Framework 13 AMD Ryzen AI 300
```
> sudo framework_tool.exe --pd-info
Left / Ports 01
  Silicon ID:     0x3580
  Mode:           MainFw
  Flash Row Size: 256 B
  Bootloader Version:   Base: 3.6.0.009,  App: 0.0.01
  FW1 (Backup) Version: Base: 3.7.0.197,  App: 0.0.0B
  FW2 (Main)   Version: Base: 3.7.0.197,  App: 0.0.0B
Right / Ports 23
  Silicon ID:     0x3580
  Mode:           MainFw
  Flash Row Size: 256 B
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
