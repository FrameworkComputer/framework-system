# Example usage

## Check firmware versions

### BIOS (Mainboard, UEFI, EC, PD)

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
  Build version:  "lilac-3.0.0-1541dc6 2025-05-05 11:31:24 zoid@localhost"
  RO Version:     "lilac-3.0.0-1541dc6"
  RW Version:     "lilac-3.0.0-1541dc6"
  Current image:  RO
PD Controllers
  Right (01)
    Main:       0.0.0E (Active)
    Backup:     0.0.0E
  Left  (23)
    Main:       0.0.0E (Active)
    Backup:     0.0.0E
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
  USI Protocol:     false
  MPP Protocol:     true
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
  IC Type:           0239
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
ALS:    0 Lux
Accelerometers:
  Lid Angle:  122 Deg
  Sensor 1:   X=+0.00G Y=+0.84G, Z=+0.52G
  Sensor 2:   X=-0.03G Y=+0.00G, Z=+1.01G
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
```

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
