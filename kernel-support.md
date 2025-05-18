# ChromeOS Kernel Modules

Much of the functionality of `framework_tool` is to interact with the EC.
On Linux there are kernel modules to do similar tasks

## EC Version

```
> cat /sys/class/chromeos/cros_ec/version
RO version:    lilac-3.0.3-413f018
RW version:    lilac-3.0.3-413f018
Firmware copy: RO
Build info:    lilac-3.0.3-413f018 2025-03-06 05:45:28 marigold2@ip-172-26-3-226
Chip vendor:   Nuvoton
Chip name:     npcx9m3f
Chip revision: 00160207
Board version: 8
```

### Reboot EC

```
> cat /sys/class/chromeos/cros_ec/reboot
ro|rw|cancel|cold|disable-jump|hibernate|cold-ap-off [at-shutdown]
```

## Sensors (Fan Speed) and Temperature

```
> cat /sys/class/chromeos/cros_ec/device/cros-ec-hwmon.8.auto/hwmon/hwmon13/temp*
> sensors cros_ec-isa-0000
cros_ec-isa-0000
Adapter: ISA adapter
fan1:               0 RPM
local_f75303@4d:  +42.9°C
cpu_f75303@4d:    +46.9°C
ddr_f75303@4d:    +34.9°C
cpu@4c:           +48.9°C

> ls -l /sys/class/chromeos/cros_ec/device/cros-ec-hwmon.*.auto/hwmon/hwmon*/
total 0
lrwxrwxrwx 1 root root    0 May 18 13:47 device -> ../../../cros-ec-hwmon.8.auto/
-r--r--r-- 1 root root 4096 May 18 13:47 fan1_fault
-r--r--r-- 1 root root 4096 May 18 13:47 fan1_input
-r--r--r-- 1 root root 4096 May 18 17:40 name
drwxr-xr-x 2 root root    0 May 18 17:37 power/
lrwxrwxrwx 1 root root    0 May 18 13:52 subsystem -> ../../../../../../../class/hwmon/
-r--r--r-- 1 root root 4096 May 18 13:47 temp1_fault
-r--r--r-- 1 root root 4096 May 18 17:42 temp1_input
-r--r--r-- 1 root root 4096 May 18 17:42 temp1_label
-r--r--r-- 1 root root 4096 May 18 13:47 temp2_fault
-r--r--r-- 1 root root 4096 May 18 13:47 temp2_input
-r--r--r-- 1 root root 4096 May 18 17:42 temp2_label
-r--r--r-- 1 root root 4096 May 18 13:47 temp3_fault
-r--r--r-- 1 root root 4096 May 18 13:47 temp3_input
-r--r--r-- 1 root root 4096 May 18 17:42 temp3_label
-r--r--r-- 1 root root 4096 May 18 13:47 temp4_fault
-r--r--r-- 1 root root 4096 May 18 13:47 temp4_input
-r--r--r-- 1 root root 4096 May 18 17:42 temp4_label
-rw-r--r-- 1 root root 4096 May 18 13:52 uevent
```

## Keyboard backlight LEDs (Framework 12, Framework 13)

Module: `leds_cros_ec`

```
# Check current brightness
# This changes even if you adjust with fn+space
> cat /sys/class/leds/chromeos::kbd_backlight/brightness
0

# Check max brightness
> cat /sys/class/leds/chromeos::kbd_backlight/max_brightness
100

# Change brightness
> echo 100 | sudo tee /sys/class/leds/chromeos::kbd_backlight/brightness
100
```
