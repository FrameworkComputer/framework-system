# Advanced debugging

## Verbosity

To debug, increase the verbosity from the commandline with `-v`.
The verbosity levels are:

| Commandline | Level  |
|-------------|--------|
| `-q`        | No log |
| None        | Error  |
| `-v`        | Warn   |
| `-vv`       | Info   |
| `-vvv`      | Debug  |
| `-vvvv`     | Trace  |

For example it is useful to check which EC driver is used:

```
> framework_tool --kblight -vvv
[DEBUG] Chromium EC Driver: CrosEc
[DEBUG] send_command(command=0x22, ver=0, data_len=0)
Keyboard backlight: 0%

> framework_tool --driver portio --kblight -vvv
[DEBUG] Chromium EC Driver: Portio
[DEBUG] send_command(command=0x22, ver=0, data_len=0)
Keyboard backlight: 0%
```

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

## Check EFI Resource Table

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

## Intel ME Information

Show Intel Management Engine status, including firmware version, bootguard
configuration, and security status. This information is read from SMBIOS tables.

On Linux, additional information is available from sysfs.

```
> sudo framework_tool --meinfo
Intel ME Information (from sysfs)
  Enabled:          true
  Firmware Version: 0:18.0.15.2518
  Recovery Version: 0:18.0.15.2518
  FITC Version:     0:18.0.5.2141
  ME Family:        CSME 18+
Intel ME Status (SMBIOS Type 0xDB)
  Working State:    Normal
  Operation Mode:   Normal
  Bootguard:
    Enabled:        Yes
    ACM Active:     Yes
    ACM Done:       Yes
    FPF SOC Lock:   No
```

Use `-v` for verbose output including raw HFSTS registers and SMBIOS handle information:

```
> sudo framework_tool --meinfo -v
Intel ME Information (from sysfs)
  Enabled:          true
  Firmware Version: 0:18.0.15.2518
  Recovery Version: 0:18.0.15.2518
  FITC Version:     0:18.0.5.2141
  ME Family:        CSME 18+
ME Handles (from SMBIOS Type 14):
  Handle: 0x002D
  Handle: 0x002E
  Handle: 0x002F
  Handle: 0x0030
  Handle: 0x0031
  Handle: 0x0032
  Handle: 0x0033
Intel ME Status (SMBIOS Type 0xDB)
  Working State:    Normal
  Operation Mode:   Normal
  Bootguard:
    Enabled:        Yes
    ACM Active:     Yes
    ACM Done:       Yes
    FPF SOC Lock:   No
  HFSTS Registers:
    HFSTS1: 0xA0000255
    HFSTS2: 0x82108908
    HFSTS3: 0x00000030
    HFSTS4: 0x00000000
    HFSTS5: 0x02F41F03
    HFSTS6: 0x00000000
```

## Manually overriding tablet mode status

If you have a suspicion that the embedded controller does not control tablet
mode correctly based on Hall and G-Sensor, you can manually force a mode.

This may also be useful if you want to use the touchpad and keyboard while the
lid is folded back - for example if you're using an external display only (Cyberdeck).
In this case you can force laptop mode.

Tablet mode:
- Sets a GPIO connected to the touchpad to disable it
- Stops the EC from sending keypresses to the CPU

```
# Force tablet mode to disable touchpad and keyboard
> framework_tool --tablet-mode tablet

# Force laptop mode to always keep touchpad and keyboard enabled
> framework_tool --tablet-mode laptop

# Let the EC handle tablet mode automatically based on sensors
> framework_tool --tablet-mode auto
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
> framework_tool --flash-gpu-descriptor GPU FRAKMQCP41500ASSY1
> framework_tool --flash-gpu-descriptor 13 FRAKMQCP41500ASSY1
> framework_tool --flash-gpu-descriptor 0x0D FRAKMQCP41500ASSY1

# Update everything from a file
> framework_tool --flash-gpu-descriptor-file pcie_4x2.bin
```

## Analyzing binaries

### EC

Note that only since Framework 13 Intel Core Ultra (and later) the version number embedded in the ED binary is meaningful. As you can see below, in this example on Intel Core 12th/13th Gen (hx30) it's always 0.0.1.
The commit hash though is accurate and reflects the git commit it was built from.

```
> framework-tool --ec--bin ec.bin
File
  Size:                     524288 B
  Size:                        512 KB
EC
  Version:     hx30_v0.0.1-7a61a89
  RollbackVer:                   0
  Platform:                   hx30
  Version:                   0.0.1
  Commit:                  7a61a89
  Size:                       2868 B
  Size:                          2 KB
```

### PD

```
> framework_tool --pd-bin pd-0.1.14.bin
File
  Size:                      65536 B
  Size:                         64 KB
FW 1
  Silicon ID:               0x3000
  Version:                  0.1.14
  Row size:                    128 B
  Start Row:                    22
  Rows:                         95
  Size:                      12160 B
  Size:                         11 KB
FW 2
  Silicon ID:               0x3000
  Version:                  0.1.14
  Row size:                    128 B
  Start Row:                   118
  Rows:                        381
  Size:                      48768 B
  Size:                         47 KB
```

### UEFI Capsule

```
> framework_tool --capsule retimer23.cap
File
  Size:                    2232676 B
  Size:                       2180 KB
Capsule Header
  Capsule GUID: (ba2e4e6e, 3b0c, 4f25, [8a,59,4c,55,3f,c8,6e,a2])
  Header size:                  28 B
  Flags:                   0x50000
    Persist across reset  (0x10000)
    Initiate reset        (0x40000)
  Capsule Size:            2232676 B
  Capsule Size:               2180 KB
  Type:   Framework Retimer23 (Right)
```

## Raw EC Host Commands

Send an arbitrary EC host command by specifying a command ID, version, and
optional payload bytes. The response is displayed in xxd-style hex+ASCII format.

```
# Send EC_CMD_GET_VERSION (0x0002) with version 0, no payload
> sudo framework_tool --host-command 0x0002 0
Response (120 bytes):
00000000: 7375 6e66 6c6f 7765 722d 332e 302e 332d  sunflower-3.0.3-
00000010: 3838 6664 6135 3400 0000 0000 0000 0000  88fda54.........
00000020: 7375 6e66 6c6f 7765 722d 332e 302e 332d  sunflower-3.0.3-
00000030: 3838 6664 6135 3400 0000 0000 0000 0000  88fda54.........
00000040: 0000 0000 0000 0000 0000 0000 0000 0000  ................
00000050: 0000 0000 0000 0000 0000 0000 0000 0000  ................
00000060: 0100 0000 0000 0000 0000 0000 0000 0000  ................
00000070: 0000 0000 0000 0000                      ........

# Query supported versions of EC_CMD_GET_VERSION (0x0002)
# EC_CMD_GET_CMD_VERSIONS (0x0008), version 0, payload: command ID byte
# Command 2 supports version 0 and 1 (0b11 = 3)
> framework_tool --host-command 0x0008 0 2
Response (24 bytes):
00000000: 0300 0000 0000 0000 0000 0000 0000 0000  ................
00000010: 0000 0000 0000 0000                      ........
# Command 1 only supports supports version 0 (0b01 = 1)
> framework_tool --host-command 0x0008 0 1
Response (24 bytes):
00000000: 0100 0000 0000 0000 0000 0000 0000 0000  ................
00000010: 0000 0000 0000 0000                      ........
```

## Version Check

Check if the firmware version is what you expect, returns exit code 0 on
succcess, 1 on failure.

```
# Check which devices it's available for
> ./framework_tool --device
  [possible values: bios, ec, pd0, pd1, rtm01, rtm23, ac-left, ac-right]

For more information try '--help'

# Successful compare
> ./framework_tool --device bios --compare-version 03.01
Target Version "03.01"
Comparing BIOS version "03.01"
Compared version:   0
> echo $?
0

# Failed compare
> ./framework_tool --device bios --compare-version 03.00
    Finished dev [unoptimized + debuginfo] target(s) in 0.05s
Target Version "03.00"
Comparing BIOS version "03.01"
Compared version:   1
Error: "Fail"

> echo $?
1
```

On UEFI Shell:

```
# Check if AC is attached on left side
Shell> fs0:framework_tool.efi --device ac-left --compare-version 1
Target Version "1"
Comparing AcLeft "1"
Comparison Result: 0
# It is
Shell> echo %lasterror%
0x0

# Check if AC is attached on right side
Shell> fs0:framework_tool.efi --device ac-right --compare-version 1
Target Version "1"
Comparing AcLeft "0"
Comparison Result: 1

# It is not
Shell> echo %lasterror%
0x1
```
