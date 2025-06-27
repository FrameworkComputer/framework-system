# Example usage

## Check firmware versions

### BIOS (Mainboard, UEFI, EC, PD, Retimer)

Example on Framework 13 AMD Ryzen AI 300 Series:

```
> framework_tool --versions
Mainboard Hardware
  Type:           Laptop 13 (AMD Ryzen AI 300 Series)
  Revision:       MassProduction
UEFI BIOS
  Version:        03.00
  Release Date:   03/10/2025
EC Firmware
  Build version:  lilac-3.0.0-1541dc6 2025-05-05 11:31:24 zoid@localhost
  Current image:  RO
PD Controllers
  Right (01):       0.0.0E (MainFw)
  Left  (23):       0.0.0E (MainFw)
[...]
```

Example on Framework 13 Intel Core Ultra Series 1:

```
> framework_tool --versions
Mainboard Hardware
  Type:           Laptop 13 (AMD Ryzen AI 300 Series)
  Revision:       MassProduction
UEFI BIOS
  Version:        03.03
  Release Date:   10/07/2024
EC Firmware
  Build version:  marigold-3.0.3-278d300 2024-10-04 03:03:58 marigold1@ip-172-26-3-226
  Current image:  RO
PD Controllers
  Right (01):       0.0.08 (MainFw)
  Left  (23):       0.0.08 (MainFw)
[...]
```

### Camera (Framework 12, Framework 13, Framework 16)

Example on Framework 12:

```
> framework_tool --versions
[...]
Framework Laptop 12 Webcam Module
  Firmware Version: 0.1.6
```

Example on Framework 13:

```
> framework_tool --versions
[...]
Laptop Webcam Module (2nd Gen)
  Firmware Version: 1.1.1
```

### Touchscreen (Framework 12)

```
> framework_tool --versions
[...]
Touchscreen
  Firmware Version: v7.0.0.5.0.0.0.0
  Protocols:        USI
```

### Stylus (Framework 12)

```
> sudo framework_tool --versions
[...]
Stylus
  Serial Number:    28C1A00-12E71DAE
  Vendor ID:        32AC (Framework Computer)
  Product ID:       002B (Framework Stylus)
  Firmware Version: FF.FF
[...]
```

### Touchpad (Framework 12, Framework 13, Framework 16)

```
> framework_tool --versions
[...]
Touchpad
  Firmware Version: v0E07
```

### Input modules (Framework 16)

Shows firmware version and location of the modules.

```
> framework_tool --versions
[...]
Laptop 16 Numpad
  Firmware Version: 0.2.9
  Location: [X] [ ] [ ]       [ ] [ ]
Laptop 16 ANSI Keyboard
  Firmware Version: 0.2.9
  Location: [ ] [ ] [X]       [ ] [ ]
[...]
```

```
> framework_tool --versions
[...]
LED Matrix
  Firmware Version: 0.2.0
  Location: [X] [ ] [ ]       [ ] [ ]
Laptop 16 ANSI Keyboard
  Firmware Version: 0.2.9
  Location: [ ] [x] [ ]       [ ] [ ]
LED Matrix
  Firmware Version: 0.2.0
  Location: [ ] [ ] [ ]       [ ] [x]
[...]
```

### DisplayPort or HDMI Expansion Card

```
> framework_tool --dp-hdmi-info
DisplayPort Expansion Card
  Serial Number:        11AD1D0030123F17142C0B00
  Active Firmware:      101 (3.0.11.065)
  Inactive Firmware:    008 (3.0.11.008)
  Operating Mode:       MainFw (#2)

# Or
> framework_tool --versions
[...]
DisplayPort Expansion Card
  Active Firmware:      101 (3.0.11.065)
  Inactive Firmware:    008 (3.0.11.008)
  Operating Mode:       MainFw (#2)
```

### CSME Version (Linux on Intel systems)

```
> framework_tool --versions
[...]
CSME
  Firmware Version: 0:16.1.32.2473
[...]
```

### Firmware Version using ESRT (BIOS, Retimer, CSME)

All systems have at least an entry for BIOS. Intel systems also have CSME and some Retimers.

Example on Framework 13 Intel Core Ultra Series 1:

```
> sudo framework_tool --esrt
ESRT Table
  ResourceCount:        4
  ResourceCountMax:     4
  ResourceVersion:      1
ESRT Entry 0
  GUID:                 BDFFCE36-809C-4FA6-AECC-54536922F0E0
  GUID:                 MtlRetimer23
  Type:                 DeviceFirmware
  Version:              0x270 (624)
  Min FW Version:       0x0 (0)
  Capsule Flags:        0x0
  Last Attempt Version: 0x270 (624)
  Last Attempt Status:  Success
ESRT Entry 1
  GUID:                 32D8D677-EEBC-4947-8F8A-0693A45240E5
  GUID:                 MtlCsme
  Type:                 DeviceFirmware
  Version:              0x85D (2141)
  Min FW Version:       0x3E8 (1000)
  Capsule Flags:        0x0
  Last Attempt Version: 0x0 (0)
  Last Attempt Status:  Success
ESRT Entry 2
  GUID:                 C57FD615-2AC9-4154-BF34-4DC715344408
  GUID:                 MtlRetimer01
  Type:                 DeviceFirmware
  Version:              0x270 (624)
  Min FW Version:       0x0 (0)
  Capsule Flags:        0x0
  Last Attempt Version: 0x270 (624)
  Last Attempt Status:  Success
ESRT Entry 3
  GUID:                 72CECB9B-2B37-5EC2-A9FF-C739AABAADF3
  GUID:                 MtlBios
  Type:                 SystemFirmware
  Version:              0x303 (771)
  Min FW Version:       0x303 (771)
  Capsule Flags:        0x0
  Last Attempt Version: 0x303 (771)
  Last Attempt Status:  Success
```

## Check input deck status

### On Framework 12

```
> framework_tool --inputdeck
Input Deck
  Chassis Closed:      true
  Power Button Board:  Present
  Audio Daughterboard: Present
  Touchpad:            Present
```

### On Framework 13

```
> framework_tool --inputdeck
Input Deck
  Chassis Closed:      true
  Audio Daughterboard: Present
  Touchpad:            Present
```

### On Framework 16

```
> framework_tool --inputdeck
Chassis Closed:   true
Input Deck State: On
Touchpad present: true
SLEEP# GPIO high: true
Positions:
  Pos 0: GenericC
  Pos 1: KeyboardA
  Pos 2: Disconnected
  Pos 3: Disconnected
  Pos 4: GenericC
```

## Check temperatures and fan speed

```
> sudo framework_tool --thermal
  F75303_Local: 43 C
  F75303_CPU:   44 C
  F75303_DDR:   39 C
  APU:          62 C
  Fan Speed:       0 RPM
```

## Check sensors

### Ambient Light (Framework 13, Framework 16)

```
> sudo framework_tool --sensors
ALS:   76 Lux
```

### Accelerometer (Framework 12)

```
> sudo framework_tool --sensors
Accelerometers:
  Lid Angle:   118 Deg
  Lid Sensor:  X=+0.00G Y=+0.86G, Z=+0.53G
  Base Sensor: X=-0.03G Y=-0.07G, Z=+1.02G
```

## Set custom fan duty/RPM

```
# Set a target fanduty of 100% (all or just fan ID=0)
> sudo framework_tool --fansetduty 100
> sudo framework_tool --fansetduty 0 100
> sudo framework_tool --thermal
  F75303_Local: 40 C
  F75303_CPU:   41 C
  F75303_DDR:   37 C
  APU:          42 C
  Fan Speed:    7281 RPM

# Set a target RPM (all or just fan ID=0)
> sudo framework_tool --fansetrpm 3141
> sudo framework_tool --fansetrpm 0 3141
> sudo framework_tool --thermal
  F75303_Local: 41 C
  F75303_CPU:   42 C
  F75303_DDR:   37 C
  APU:          44 C
  Fan Speed:    3171 RPM

# And back to normal
> sudo framework_tool --autofanctrl
> sudo framework_tool --thermal
  F75303_Local: 40 C
  F75303_CPU:   40 C
  F75303_DDR:   38 C
  APU:          42 C
  Fan Speed:       0 RPM
```

## Check expansion bay (Framework 16)

```
> sudo framework_tool --expansion-bay
Expansion Bay
  Enabled:       true
  No fault:      true
  Door  closed:  true
  Board:         DualInterposer
  Serial Number: FRAXXXXXXXXXXXXXXX
  Config:        Pcie4x2
  Vendor:        SsdHolder
  Expansion Bay EEPROM
    Valid:       true
    HW Version:  8.0
```

Add `-vv` for more verbose details.

## Check charger and battery status (Framework 12/13/16)

```
> sudo framework_tool --power
Charger Status
  AC is:            not connected
  Charger Voltage:  17048mV
  Charger Current:  0mA
  Chg Input Current:384mA
  Battery SoC:      93%
Battery Status
  AC is:            not connected
  Battery is:       connected
  Battery LFCC:     3693 mAh (Last Full Charge Capacity)
  Battery Capacity: 3409 mAh
                    58.96 Wh
  Charge level:     92%
  Battery discharging
```

Get more information

```
> sudo framework_tool --power -vv
Charger Status
  AC is:            not connected
  Charger Voltage:  14824mV
  Charger Current:  0mA
  Chg Input Current:384mA
  Battery SoC:      33%
Battery Status
  AC is:            not connected
  Battery is:       connected
  Battery LFCC:     4021 mAh (Last Full Charge Capacity)
  Battery Capacity: 1300 mAh
                    19.267 Wh
  Charge level:     32%
  Manufacturer:     NVT
  Model Number:     FRANGWA
  Serial Number:    038F
  Battery Type:     LION
  Present Voltage:  14.821 V
  Present Rate:     943 mA
  Design Capacity:  3915 mAh
                    60.604 Wh
  Design Voltage:   15.480 V
  Cycle Count:      64
  Battery discharging
```

### Setting a custom charger current limit

```
# 1C = normal charging rate
# This means charging from 0 to 100% takes 1 hour
# Set charging rate to 0.8C
> sudo framework_tool --charge-rate-limit 0.8

# Limit charge current to the battery to to 2A
# In the output of `framework_tool --power -vv` above you can se "Design Capacity"
# Dividing that by 1h gives you the maximum charging current (1C)
# For example Design Capacity:  3915 mAh => 3915mA
> sudo framework_tool --charge-current-limit 2000

# And then plug in a power adapter
> sudo framework_tool --power
Charger Status
  AC is:            connected
  Charger Voltage:  17800mV
  Charger Current:  2000mA
                    0.51C
  Chg Input Current:3084mA
  Battery SoC:      87%
Battery Status
  AC is:            connected
  Battery is:       connected
  Battery LFCC:     3713 mAh (Last Full Charge Capacity)
  Battery Capacity: 3215 mAh
                    56.953 Wh
  Charge level:     86%
  Battery charging

# Remove limit (set rate to 1C)
> sudo framework_tool --charge-rate-limit 1

# Back to normal
> sudo framework_tool --power
Charger Status
  AC is:            connected
  Charger Voltage:  17800mV
  Charger Current:  2740mA
                    0.70C
  Chg Input Current:3084mA
  Battery SoC:      92%
Battery Status
  AC is:            connected
  Battery is:       connected
  Battery LFCC:     3713 mAh (Last Full Charge Capacity)
  Battery Capacity: 3387 mAh
                    60.146 Wh
  Charge level:     91%
  Battery charging

# Set charge rate/current limit only if battery is >80% charged
> sudo framework_tool --charge-rate-limit 0.8 80
> sudo framework_tool --charge-current-limit 2000 80
```

## EC Console

```
# Get recent EC console logs and watch for more
> framework_tool.exe --console follow
[53694.741000 Battery 62% (Display 61.1 %) / 3h:18 to empty]
[53715.010000 Battery 62% (Display 61.0 %) / 3h:21 to empty]
[53734.281200 Battery 62% (Display 60.9 %) / 3h:18 to empty]
[53738.037200 Battery 61% (Display 60.9 %) / 3h:6 to empty]
[53752.301500 Battery 61% (Display 60.8 %) / 3h:15 to empty]
```

## Keyboard backlight

```
# Check current keyboard backlight brightness
> framework_tool.exe --kblight
Keyboard backlight: 5%

# Set keyboard backlight brightness
# Off
> framework_tool.exe --kblight 0
# 20%
> framework_tool.exe --kblight 20
```

## Fingerprint/Powerbutton brightness

On Framework 13 and Framework 16 the power button has an integrated fingerprint reader, hence the name.
On Framework 12 it does not, but the same command can be used.

```
# Check the current brightness
> framework_tool --fp-brightness
Fingerprint LED Brightness
  Requested:  Auto
  Brightness: 55%

# Set it to a custom perfentage
> framework_tool --fp-brightness 42
Fingerprint LED Brightness
  Requested:  Custom
  Brightness: 42%

# Set to a specific level (like the BIOS setting does)
> framework_tool --fp-led-level high
Fingerprint LED Brightness
  Requested:  High
  Brightness: 55%

# Set it back to auto
> framework_tool --fp-led-level auto
Fingerprint LED Brightness
  Requested:  Auto
  Brightness: 15%
```

## RGB LED (Framework Desktop)

```
# To set three LEDs to red, green, blue
sudo framework_tool --rgbkbd 0 0xFF0000 0x00FF00 0x0000FF

# To clear 8 LEDs
sudo framework_tool --rgbkbd 0 0 0 0 0 0 0 0 0

# Just turn the 3rd LED red
sudo framework_tool --rgbkbd 2 0xFF0000
```

## Stylus (Framework 12)

```
> sudo framework_tool --stylus-battery
Stylus Battery Strength: 77%
```

## Remap keyboard

The below information should be correct for all current (as of 06/2025) Intel and AMD Framework 13's.

For the Framework 16, use [VIA](https://keyboard.frame.work).

For the Framework 12, there is a different keyboard matrix, but the scancodes are the same.

### General Instructions

To remap keys on your keyboard, you will run the command:

```
sudo framework_tool --remap-key [Y] [X] [SCANCODE]
```

-   **Y** is the row of the keyboard matrix for the key that you want to remap
-   **X** is the column of the keyboard matrix for the key that you want to remap
-   **SCANCODE** is the scancode that you want the key to send

### Specific Examples

#### FW13

Set **Caps Lock** to be **Esc**:

```
> sudo framework_tool --remap-key 4 4 0x0076
```

Set **Enter** to be **Enter** (for example to fix it if you broke it):

```
> sudo framework_tool --remap-key 1 14 0x005a
```

Swap **L_Alt** and **L_Control**:

```
> sudo framework_tool --remap-key 1 12 0x0011
> sudo framework_tool --remap-key 1 3 0x0014
```

#### FW12

set **Caps Lock** to be **Esc**:

```
> sudo framework_tool --remap-key 6 15 0x0076
```

Swap **L_Alt** and **L_Control**:

```
> sudo framework_tool --remap-key 1 14 0x0011
> sudo framework_tool --remap-key 6 13 0x0014
```

### Finding Keycodes and Matrix Addresses

-   Mr. Howett put together a wonderul [map of the matrix](https://www.howett.net/data/framework_matrix/) for the FW13 here.
-   The actual embedded controller source code contains a table that looks just the same to the matrix (albiet rotated). It can be found [here](https://github.com/FrameworkComputer/EmbeddedController/blob/f6620a8200e8d1b349078710b271540b5b8a1a18/board/hx30/keyboard_customization.c#L25).
-   Some scancodes can be found in the embedded controller [source code](https://github.com/FrameworkComputer/EmbeddedController/blob/f6620a8200e8d1b349078710b271540b5b8a1a18/include/keyboard_8042_sharedlib.h#L106) and some can be found in [the original list from the kernel](http://kbd-project.org/docs/scancodes/scancodes-10.html#ss10.6).
    -   A full breakdown of all possible scancodes is [available](http://kbd-project.org/docs/scancodes/scancodes-1.html) but won't be useful to most users.

#### Scancodes

|ScanCode  |    Key     |  ScanCode |       Key    |     ScanCode   |        Key          | ScanCode  |          Key         |
|----------|------------|-----------|--------------|----------------|---------------------|-----------|----------------------|
|0x000e    |   ` ~      |0x004b     |      L       |    0x0075      |   KP-8 / Up         | 0x0017    |          F14         |
|0x0016    |    1 !     |  0x004c   |       ; :    |      0x0073    |        KP-5         | 0x001f    |          F15         |
|0x001e    |    2 @     |  0x0052   |      ' "     |    0x0072      |  KP-2 / Down        | 0xe038    |         BACK         |
|0x0026    |   3 #      | 0x0000    |   non-US-1   |     0x0070     |    KP-0 / Ins       |  0xe020   |         REFRESH      |
|0x0025    |   4 $      | 0x005a    |     Enter    |     0x007c     |      KP-*           | 0xe030    |        FORWARD       |
|0x002e    |    5 %     |  0x0012   |     LShift   |      0x007d    |    KP-9 / PgUp      | 0xe01d    |      FULLSCREEN      |
|0x0036    |   6 ^      | 0x001a    |       Z      |     0x0074     |   KP-6 / Right      |  0xe024   |        OVERVIEW      |
|0x003d    |    7 &     |  0x0022   |        X     |      0x007a    |    KP-3 / PgDn      | 0xe02d    |       SNAPSHOT       |
|0x003e    |   8 *      | 0x0021    |       C      |     0x0071     |    KP-. / Del       |  0xe02c   |     BRIGHTNESS_DOWN  |
|0x0046    |    9 (     |  0x002a   |        V     |      0x007b    |       KP--          |  0xe035   |      BRIGHTNESS_UP   |
|0x0045    |    0 )     |  0x0032   |        B     |      0x0079    |        KP-+         | 0xe03c    |  PRIVACY_SCRN_TOGGLE |
|0x004e    |   - _      |0x0031     |      N       |  0x00e0-5a     |    KP-Enter         | 0xe023    |      VOLUME_MUTE     |
|0x0055    |    = +     |  0x003a   |        M     |      0x0076    |        Esc          | 0xe021    |      VOLUME_DOWN     |
|0x0066    | Backspace  |  0x0041   |      , <     |     0x0005     |        F1           |  0xe032   |        VOLUME_UP     |
|0x000d    |    Tab     |  0x0049   |      . >     |     0x0006     |        F2           |  0xe043   |    KBD_BKLIGHT_DOWN  |
|0x0015    |     Q      |  0x004a   |       / ?    |      0x0004    |         F3          | 0xe044    |    KBD_BKLIGHT_UP    |
|0x001d    |     W      |  0x0059   |     RShift   |      0x000c    |         F4          | 0xe04d    |      NEXT_TRACK      |
|0x0024    |     E      |  0x0014   |      LCtrl   |      0x0003    |         F5          | 0xe015    |      PREV_TRACK      |
|0x002d    |     R      |  0x0011   |      LAlt    |      0x000b    |         F6          | 0xe054    |      PLAY_PAUSE      |
|0x002c    |     T      |  0x0029   |      space   |      0x0083    |         F7          | 0xe075    |          UP          |
|0x0035    |     Y      |  0x00e0-11|      RAlt    |      0x000a    |         F8          | 0xe072    |         DOWN         |
|0x003c    |     U      |  0x00e0-14|      RCtrl   |      0x0001    |         F9          | 0xe06b    |         LEFT         |
|0x0043    |     I      |  e0-70    |     Insert   |      0x0009    |        F10          | 0xe074    |         RIGHT        |
|0x0044    |     O      |  e0-71    |     Delete   |      0x0078    |        F11          | 0x0014    |       LEFT_CTRL      |
|0x004d    |     P      |  e0-6c    |      Home    |      0x0007    |        F12          | 0xe014    |      RIGHT_CTRL      |
|0x0054    |   [ {      | e0-69     |      End     |   0x00e0-7c    |      PrtScr         |  0x0011   |        LEFT_ALT      |
|0x005b    |   ] }      | e0-7d     |     PgUp     |     0x0084     |    Alt+SysRq        |  0xe011   |        RIGHT_ALT     |
|0x005d    |   \ \|      |  e0-7a    |    PgDn      |    0x007e      |   ScrollLock        |   0xe01f  |         LEFT_WIN     |
|0x0058    | CapsLock   |  e0-6b    |      Left    |   0x00e1-14-77 |       Pause         | 0xe027    |       RIGHT_WIN      |
|0x001c    |     A      |  e0-75    |       Up     |    0x00e0-7e   |     Ctrl+Break      | 0xe02f    |         MENU         |
|0x001b    |     S      |  e0-72    |      Down    |    0x00e0-1f   |  LWin (USB: LGUI)   | 0xe037    |         POWER        |
|0x0023    |     D      |  e0-74    |      Right   |    0x00e0-27   |  RWin (USB: RGUI)   | 0x0077    |        NUMLOCK       |
|0x002b    |     F      |  0x0077   |     NumLock  |    0x00e0-2f   |        Menu         | 0x0058    |       CAPSLOCK       |
|0x0034    |     G      |  0x006c   |   KP-7 / Home|    0x00e0-3f   |       Sleep         | 0x007e    |      SCROLL_LOCK     |
|0x0033    |     H      |  0x006b   |   KP-4 / Left|    0x00e0-37   |       Power         | 0xe07e    |      CTRL_BREAK      |
|0x003b    |     J      |  0x0069   |   KP-1 / End |      e0-5e     |        Wake         | 0xe076    |       RECOVERY       |
|0x0042    |     K      |  0x00e0-4a|      KP-/    |      0x000f    |        F13

#### Wiring Matrix

##### FrameWork 13

> Matrix w/ keys

|   |  0  |    1     |    2     |  3   |       4       |      5       | 6 | 7 |       8     |    9   |        10       |         11        |   12  |       13      |        14      |        15      |
|---|-----|----------|----------|------|---------------|--------------|---|---|-------------|--------|-----------------|-------------------|-------|---------------|----------------|----------------|
| 0 |  c  |  Delete  |    q     | RAlt |   KP Enter    |      x       |v  |m  |      .      | RShift |      Comma      | Katakana Hiragana | RCtrl |       /       |       '        |     Yen        |
| 1 | KP- |  KP Ins  |   KP0    | LAlt |     Space     |      z       |b  |n  |    Down     | LShift |      KP*        |     Henkan        |LCtrl  |     Up        |    Enter       |Bright. Up F8   |
| 2 | KP+ |   KP9    |    Fn    |      |       e       | Vol. Down F2 |g  |h  |     \       |        |Bright. Down F7  |       KP8         |       |     -         |Scan Code e016  |    Right       |
| 3 | KP2 |  LMeta   |   Tab    |      | Audio Prev F4 |   Mute F1    |t  |y  |      o      |        |  Audio Next F6  |    Project F9     |       | Framework F12 |      End       | Scan Code e01a |
| 4 | KP3 |   KP7    |    `     |      |  Caps Lock    |      s       |5  |6  | RF Kill F10 |        |  Play Pause F5  |      Ro Kana      |       |       0       |    + =         |                |
| 5 | KP. |   Home   |    1     |      |       3       |      2       |4  |7  |      9      |        |        8        |       102nd       |       |       p       |       BS       |      KP4       |
| 6 | KP1 | Page Up  | Muhenkan |      |  Vol. Up F3   |      w       |r  |u  | PrtScr F11  |        |        i        |       Left        |       |      [        |      ]         |    KP5         |
| 7 | KP/ | Num Lock |    a     |      |   Page Down   |    Escape    |f  |j  |      l      |        |        k        |       Menu        |       |       ;       |       d        |      KP6       |

> Matrix w/ scancodes

|    |   0    |   1    |   2    |   3    |   4    |   5    |   6    |   7    |   8    |   9    |   10   |   11   |   12   |   13   |   14   |   15   |
| ---|--------|--------|--------|--------|--------|--------|--------|--------|--------|--------|--------|--------|--------|--------|--------|--------|
|  0 | 0x0021 | 0xe071 | 0x0015 | 0xe011 | 0xe05a | 0x0022 | 0x002A | 0x003a | 0x0049 | 0x0059 | 0x0041 | 0x0013 | 0xe014 | 0x004a | 0x0052 | 0x006a |
|  1 | 0x007B | 0xe070 | 0x0070 | 0x0011 | 0x0029 | 0x001a | 0x0032 | 0x0031 | 0xe072 | 0x0012 | 0x007c | 0x0064 | 0x0014 | 0xe075 | 0x005a | 0x000a |
|  2 | 0x0079 | 0x007D | 0x00ff | 0x0000 | 0x0024 | 0x0006 | 0x0034 | 0x0033 | 0x005d | 0x0000 | 0x0083 | 0x0075 | 0x0000 | 0x004e | 0xe03c | 0xe074 |
|  3 | 0x0072 | 0xe01f | 0x000D | 0x0000 | 0x000c | 0x0005 | 0x002c | 0x0035 | 0x0044 | 0x0000 | 0x000b | 0x0001 | 0x0000 | 0x0007 | 0xe069 | 0xe054 |
|  4 | 0x007A | 0x006c | 0x000E | 0x0000 | 0x0058 | 0x001b | 0x002e | 0x0036 | 0x0009 | 0x0000 | 0x0003 | 0x0051 | 0x0000 | 0x0045 | 0x0055 | 0x0000 |
|  5 | 0x0071 | 0xe06c | 0x0016 | 0x0000 | 0x0026 | 0x001e | 0x0025 | 0x003d | 0x0046 | 0x0000 | 0x003e | 0x0061 | 0x0000 | 0x004d | 0x0066 | 0x006b |
|  6 | 0x0069 | 0xe07d | 0x0067 | 0x0000 | 0x0004 | 0x001d | 0x002d | 0x003c | 0x0078 | 0x0000 | 0x0043 | 0xe06b | 0x0000 | 0x0054 | 0x005b | 0x0073 |
|  7 | 0xe04A | 0x0077 | 0x001c | 0x0000 | 0xe07a | 0x0076 | 0x002b | 0x003b | 0x004b | 0x0000 | 0x0042 | 0xe02f | 0x0000 | 0x004c | 0x0023 | 0x0074 |

##### FrameWork 12

If anyone can find the source code with the FW12 matrix please let me know and I'll add info about that to this page. I don't have an FW12 so all my testing was done on an FW13.


## Advanced commands

Mostly for debugging firmware.

See [EXAMPLES_ADVANCED.md](EXAMPLES_ADVANCED.md)
