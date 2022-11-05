# Framework System

Rust libraries and tools to interact with the system.

Features:

- [x] All-In-One Tool (`framework_tool`)
  - [x] Tested on Linux
  - [ ] Tested on Windows
  - [ ] Tested on UEFI Shell
  - [ ] Shows Firmware Binary Information
    - [x] EC
    - [ ] CCG5 PD (11th Gen TigerLake)
    - [ ] CCG6 PD (12th Gen AlderLake)

## Prerequisites

Only [Rustup](https://rustup.rs/) is needed. Based on `rust-toolchain.toml` it
will install the right toolchain and version for this project.

## Building

```sh
cargo build
```
