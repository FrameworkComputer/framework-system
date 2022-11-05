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
    - [x] CCG6 PD (12th Gen AlderLake)

## Prerequisites

Only [Rustup](https://rustup.rs/) is needed. Based on `rust-toolchain.toml` it
will install the right toolchain and version for this project.

## Building

```sh
# Running linter
cargo clippy

# Running autoformatter as a check
cargo fmt --check

# Fixing format issues
cargo fmt

# Building everything
cargo build
```

## Running

```
# Dumping PD FW Binary Information
>  cargo run pd pd-0.1.14.bin
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
> cargo run ec ec.bin
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
