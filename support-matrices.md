# Support Matrices

###### OS Support

| Feature                 | Linux | UEFI | Windows |
|-------------------------|-------|------|---------|
| SMBIOS                  | y     | y    | y       |
| ESRT                    | y     | y    | n       |
| Parse FW file           | y     | y    | y       |
| Parse capsule           | y     | y    | y       |
| Get EC Version          | y     | y    | y       |
| Audio Card FW Version   | y     | n    | y       |
| HDMI/DP Card FW Version | y     | n    | y       |
| ME Version              | y     | n    | y       |

###### Platform Support

| Feature          | Intel 11th Gen | Intel 12th Gen | Intel 13th Gen | Framework 13 AMD | Framework 16   |
|------------------|----------------|----------------|----------------|------------------|----------------|
| SMBIOS           | y              | y              | n              | n                | n              |
| ESRT             | y              | y              | y (not parsed) | y (not parsed)   | y              |
| EC Memory Read   | y              | y              | y              | n                | n (y UEFI)     |
| EC Communication | y              | y              | y              | n                | n (y UEFI)     |
| PD Communication | y              | y              | y              | n                | n              |
| Parse PD Binary  | y              | y              | y              | y                | y              |

###### Dependencies

| Command                  | Depends on       | Platforms         |
|--------------------------|------------------|-------------------|
| `--version`              |                  | All               |
| `--dp-hdmi...`           | USB HID          | All               |
| `--power`                | EC Memory Read   | All               |
| `--pdports`              | EC Communication | All               |
| `--info`                 | SMBIOS           | All               |
| `--esrt`                 | ESRT             | All               |
| `--sensors`              | EC Communication | All Laptops       |
| `--fansetduty`           | EC Communication | All               |
| `--fansetrpm`            | EC Communication | All               |
| `--autofanctrl`          | EC Communication | All               |
| `--pd-info`              | PD Communication | All               |
| `--privacy`              | EC Communication | All Laptops       |
| `--intrusion`            | EC Communication | All Laptops       |
| `--inputdeck`            | EC Communication | All Laptops       |
| `--inputdeck-mode`       | EC Communication | Framework 16      |
| `--console`              | EC Communication | All               |
| `--get-gpio`             | EC Communication | All               |
| `--kblight`              | EC Communication | Framework 13      |
| `--charge-limit`         | EC Communication | All Laptops       |
| `--charge-current-limit` | EC Communication | All Laptops       |
| `--charge-rate-limit`    | EC Communication | All Laptops       |
| `--fp-led-level`         | EC Communication | All Laptops       |
| `--fp-brightness`        | EC Communication | All Laptops       |
| `--remap-key`            | EC Communication | Framework 12, 13  |
| `--rgbkbd`               | EC Communication | Framework Desktop |
| `--tablet-mode`          | EC Communication | Framework 12      |
| `--touchscreen-enable`   | I2C HID          | Framework 12      |
| `--stylus-battery        | I2C HID          | Framework 12      |
| `--kblight`              | EC Communication | Framework 13      |
