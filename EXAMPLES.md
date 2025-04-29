# Example usage


## Check temperatures and fan speed

```
> sudo ./target/debug/framework_tool --thermal
  F75303_Local: 43 C
  F75303_CPU:   44 C
  F75303_DDR:   39 C
  APU:          62 C
  Fan Speed:       0 RPM
```

## Check sensors (ALS and G-Sensor)

```
> sudo ./target/debug/framework_tool --sensors
ALS:   76 Lux
```

## Check power (AC and battery) status

```
> sudo ./target/debug/framework_tool --power
  AC is:            not connected
  Battery is:       connected
  Battery LFCC:     3949 mAh (Last Full Charge Capacity)
  Battery Capacity: 2770 mAh
                    44.729 Wh
  Charge level:     70%
  Battery discharging
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
