# Framework System

Rust libraries and tools to interact with the system.

Features:

- [x] All-In-One Tool (`framework_tool`)
  - [x] Tested on Linux
  - [x] Tested on Windows
  - [ ] Tested on UEFI Shell
  - [ ] Get firmware version from binary file
    - [x] EC (`--ec-bin`)
    - [ ] CCG5 PD (11th Gen TigerLake)
    - [x] CCG6 PD (12th Gen AlderLake) (`--pd-bin`)
  - [ ] Get firmware version from system (`--versions`)
    - [ ] BIOS
    - [x] EC
    - [ ] PD
  - [ ] Flash firmware
    - [ ] BIOS
    - [ ] EC
    - [ ] PD
  - [x] Get information about battery/AC (`--power`)
  - [x] Get information about USB-C PD ports (`--dports`)
- [ ] Implement communication with EC
  - [x] Port I/O communication on Linux
  - [ ] Port I/O communication on UEFI
  - [x] Using `cros_ec` driver in Linux kernel
  - [ ] Using DHowett's Windows CrosEC driver

## Prerequisites

Only [Rustup](https://rustup.rs/) is needed. Based on `rust-toolchain.toml` it
will install the right toolchain and version for this project.

## Building

```sh
# Running linter
cargo clippy -p framework_lib -p framework_tool

# Running autoformatter as a check
cargo fmt --check

# Fixing format issues
cargo fmt

# Building all OS tools
cargo build -p framework_lib -p framework_tool

# Build UEFI application
# Can't be built with cargo! That's why we need to exclude it in the other commands.
make -C framework_uefi
```

## Running

Run without any arguments to see the help:

```
> cargo run -p framework_tool
Swiss army knife for Framework laptops

Usage: framework_tool [OPTIONS]

Options:
  -v, --versions         List current firmware versions version
      --power            Show current power status (battery and AC)
      --pdports          Show information about USB-C PD prots
      --privacy          Show info from SMBIOS (Only on UEFI) Show privacy switch statuses (camera and microphone)
      --pd-bin <PD_BIN>  Parse versions from PD firmware binary file
      --ec-bin <EC_BIN>  Parse versions from EC firmware binary file
  -h, --help             Print help information
```

Many actions require root. First build with cargo and then run the binary with sudo:

```sh
cargo build -p framework_tool && sudo ./target/debug/framework_tool
```

Dumping version information from firmware binaries:

```
# Dumping PD FW Binary Information:
>  cargo run -p framework_tool -- --pd-bin pd-0.1.14.bin
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

# Dumping EC FW Binary Information
> cargo run -p framework_tool -- --ec--bin ec.bin
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

## Tests

- [x] Basic unit tests
- [ ] Test parsing real binaries
