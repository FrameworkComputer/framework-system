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
| ESRT             | y              | y              | y (not parsed) | y (not parsed)   | y (not parsed) |
| EC Communication | y              | y              | y              | n                | n              |
| PD Communication | y              | y              | y              | n                | n              |
| Parse PD Binary  | y              | y              | y              | y                | y              |
