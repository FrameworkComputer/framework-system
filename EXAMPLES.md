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
