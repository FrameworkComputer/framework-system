# Framework System

Rust libraries and tools to interact with the system.

The tool works on Linux, Windows and the UEFI shell.
Download it from the latest [GH Actions](https://github.com/FrameworkComputer/framework-system/actions?query=branch%3Amain) run on the main branch.
Most features are supported on every "OS". See below for details. Windows does
not yet support some features because they depend on the missing CrosEC driver
implementation.

Features:

- [x] All-In-One Tool (`framework_tool`)
  - [x] Tested on Linux
  - [ ] Tested on FreeBSD
  - [x] Tested on Windows
  - [x] Tested on UEFI Shell (`framework_uefi`)
  - [ ] Show system information
    - [x] ESRT table (UEFI and Linux only)
    - [x] SMBIOS
  - [ ] Get firmware version from binary file
    - [x] EC (`--ec-bin`)
    - [ ] CCG5 PD (11th Gen TigerLake)
    - [x] CCG6 PD (12th Gen AlderLake) (`--pd-bin`)
  - [x] Parse metadata from capsule binary
    - [x] Extract bitmap image from winux capsule to file
  - [ ] Get firmware version from system (`--versions`)
    - [x] BIOS
    - [x] EC
    - [x] PD
    - [x] ME (Only on Linux)
    - [x] Retimer
  - [ ] Flash firmware
    - [ ] BIOS
    - [ ] EC
    - [ ] PD
  - [x] Get information about battery/AC (`--power`)
  - [x] Get information about USB-C PD ports (`--pdorts`)
- [ ] Implement communication with EC
  - [x] Port I/O communication on Linux
  - [x] Port I/O communication on UEFI
  - [x] Using `cros_ec` driver in Linux kernel
  - [ ] Using DHowett's Windows CrosEC driver

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

# Building the library and tool
cargo build

# Building only the library
cargo build -p framework_lib

# Building only the tool
cargo build -p framework_tool

# Build the UEFI application
# Can't be built with cargo! That's why we need to exclude it in the other commands.
make -C framework_uefi
```

Building on Windows or in general with less features:

```ps1
# Because we're fetching a private dependency from git, it might be necessary
# to force cargo to use the git commandline. In powershell run:
$env:CARGO_NET_GIT_FETCH_WITH_CLI='true'

# Build the library and tool
cargo build --no-default-features --features "windows"

# Running the tool
cargo run --no-default-features --features "windows"
```

Cross compile from Linux to FreeBSD:

```sh
# One time, install cross tool
cargo install cross

# Make sure docker is started as well
sudo systemctl start docker

# Build
cross build --target=x86_64-unknown-freebsd --no-default-features --features unix
```

## Running

Run without any arguments to see the help:

```
> cargo run
Swiss army knife for Framework laptops

Usage: framework_tool [OPTIONS]

Options:
  -v, --versions         List current firmware versions version
      --power            Show current power status (battery and AC)
      --pdports          Show information about USB-C PD prots
      --privacy          Show info from SMBIOS (Only on UEFI) Show privacy switch statuses (camera and microphone)
      --pd-bin <PD_BIN>  Parse versions from PD firmware binary file
      --ec-bin <EC_BIN>  Parse versions from EC firmware binary file
      -t, --test         Run self-test to check if interaction with EC is possible
  -h, --help             Print help information
```

Many actions require root. First build with cargo and then run the binary with sudo:

```sh
cargo build && sudo ./target/debug/framework_tool
```

Dumping version information from firmware binaries:

```
# Dumping PD FW Binary Information:
>  cargo run -q -- --pd-bin pd-0.1.14.bin
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
> cargo run -q -- --ec--bin ec.bin
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

# Dumping Capsule Binary Information:
> cargo run -q -- --capsule retimer23.cap
File
  Size:                    2232676 B
  Size:                       2180 KB
Capsule Header
  Capsule GUID: (ba2e4e6e, 3b0c, 4f25, [8a,59,4c,55,3f,c8,6e,a2])
  Header size:                  28 B
  Flags:                   0x50000
    Persist across reset  (0x10000)
    Initiate reset        (0x40000)
  Capsule Size:            2232676 B
  Capsule Size:               2180 KB
  Type:   Framework Retimer23 (Right)
```

## Tests

- [x] Basic unit tests
- [ ] Test parsing real binaries
