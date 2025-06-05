# Framework System

Rust libraries and tools to interact with the system.

The tool works on Linux, Windows and the UEFI shell.
Download it from the latest [GH Actions](https://github.com/FrameworkComputer/framework-system/actions?query=branch%3Amain) run on the main branch.
Most features are supported on every "OS". See below for details.

## Features

To check which features are supported on which OS and platform,
see the [Support Matrices](support-matrices.md).

###### Operating System Support

- [x] OS Tool (`framework_tool`)
  - [x] Tested on Linux
  - [x] Tested on Windows
  - [x] Tested on FreeBSD
- [x] UEFI Shell tool (`framework_uefi`)

###### Firmware Information

  - [x] Show system information
    - [x] ESRT table (UEFI, Linux, FreeBSD only) (`--esrt`)
    - [x] SMBIOS
  - [x] Get firmware version from binary file
    - [x] EC (Legacy and Zephyr based) (`--ec-bin`)
    - [x] CCG5 PD (11th Gen TigerLake) (`--pd-bin`)
    - [x] CCG6 PD (Intel systems, Framework Desktop) (`--pd-bin`)
    - [x] CCG8 PD (AMD Laptops) (`--pd-bin`)
    - [x] H2O BIOS Capsule (`--h2o-capsule`)
      - [x] BIOS Version
      - [x] EC Version
      - [x] CCG5/CCG6/CCG8 PD Version
    - [x] UEFI Capsule (`--capsule`)
  - [x] Parse metadata from capsule binary
    - [x] Determine type (GUID) of capsule binary
    - [x] Extract bitmap image from winux capsule to file
  - [x] Get firmware version from system (`--versions`)
    - [x] BIOS
    - [x] EC
    - [x] PD Controller
    - [x] ME (Only on Linux)
    - [x] Retimer
    - [x] Touchpad (Linux, Windows, FreeBSD, not UEFI)
    - [x] Touchscreen (Linux, Windows, FreeBSD, not UEFI)
  - [x] Get Expansion Card Firmware (Not on UEFI so far)
    - [x] HDMI Expansion Card (`--dp-hdmi-info`)
    - [x] DisplayPort Expansion Card (`--dp-hdmi-info`)
    - [x] Audio Expansion Card (`--audio-card-info`)
  - [x] Update Expansion Card Firmware (Not on UEFI so far)
    - [x] HDMI Expansion Card (`--dp-hdmi-update`)
    - [x] DisplayPort Expansion Card (`--dp-hdmi-update`)
    - [ ] Audio Expansion Card

###### System Status

All of these need EC communication support in order to work.

- [x] Get information about battery/AC (`--power`)
- [x] Get information about USB-C PD ports (`--pdorts`)
- [x] Get information about CCGX PD Controllers (`--pd-info`)
- [x] Show status of intrusion switches (`--intrusion`)
- [x] Show status of privacy switches (`--privacy`)
- [x] Check recent EC console output (`--console recent`)

###### Changing settings

- [x] Get and set keyboard brightness (`--kblight`)
- [x] Get and set battery charge limit (`--charge-limit`)
- [x] Get and set fingerprint LED brightness (`--fp-brightness`, `--fp-led-level`)
- [x] Override tablet mode, instead of follow G-Sensor and hall sensor (`--tablet-mode`)
- [x] Disable/Enable touchscreen (`--touchscreen-enable`)

###### Communication with Embedded Controller

- [x] Framework Laptop 12 (Intel 13th Gen)
- [x] Framework Laptop 13 (Intel 11-13th Gen)
- [x] Framework Laptop 13 (AMD Ryzen 7080)
- [x] Framework Laptop 13 (AMD Ryzen AI 300)
- [x] Framework Laptop 16 (AMD Ryzen 7080)
- [x] Framework Desktop (AMD Ryzen AI Max 300)
- [x] Port I/O communication on Linux
- [x] Port I/O communication in UEFI
- [x] Port I/O communication on FreeBSD
- [x] Using `cros_ec` driver in Linux kernel
- [x] Using [Framework EC Windows driver](https://github.com/FrameworkComputer/crosecbus) based on [coolstar's](https://github.com/coolstar/crosecbus)
- [x] Using [DHowett's Windows CrosEC driver](https://github.com/DHowett/FrameworkWindowsUtils)

## Prerequisites

Only [Rustup](https://rustup.rs/) is needed. Based on `rust-toolchain.toml` it
will install the right toolchain and version for this project.

## Building

MSRV (Minimum Supported Rust Version):

- 1.74 for Linux/Windows
- 1.74 for UEFI

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
ls -l target/debug/framework_tool

# Build the UEFI application
# Can't be built with cargo! That's why we need to exclude it in the other commands.
make -C framework_uefi
ls -l framework_uefi/build/x86_64-unknown-uefi/boot.efi
```

### Dependencies

```
# NixOS
nix-shell --run fish -p cargo systemd udev hidapi pkg-config
direnv shell
```

## Install local package

```
> cargo install --path framework_tool
> which framework_tool
/home/zoid/.cargo/bin/framework_tool
```

## Running

Run without any arguments to see the help:

```
> cargo run
Swiss army knife for Framework laptops

Usage: framework_tool [OPTIONS]

Options:
  -v, --verbose...                  More output per occurrence
  -q, --quiet...                    Less output per occurrence
      --versions                    List current firmware versions version
      --esrt                        Display the UEFI ESRT table
      --power                       Show current power status (battery and AC)
      --pdports                     Show information about USB-C PD ports
      --info                        Show info from SMBIOS (Only on UEFI)
      --pd-info                     Show details about the PD controllers
      --dp-hdmi-info                Show details about connected DP or HDMI Expansion Cards
      --dp-hdmi-update <UPDATE_BIN> Update the DisplayPort or HDMI Expansion Card
      --audio-card-info             Show details about connected Audio Expansion Cards (Needs root privileges)
      --privacy                     Show privacy switch statuses (camera and microphone)
      --pd-bin <PD_BIN>             Parse versions from PD firmware binary file
      --ec-bin <EC_BIN>             Parse versions from EC firmware binary file
      --capsule <CAPSULE>           Parse UEFI Capsule information from binary file
      --dump <DUMP>                 Dump extracted UX capsule bitmap image to a file
      --h2o-capsule <H2O_CAPSULE>   Parse UEFI Capsule information from binary file
      --intrusion                   Show status of intrusion switch
      --inputdeck                   Show status of the input deck
      --input-deck-mode <INPUT_DECK_MODE>
          Set input deck power mode [possible values: auto, off, on] (Framework 16 only) [possible values: auto, off, on]
      --expansion-bay               Show status of the expansion bay (Framework 16 only)
      --charge-limit [<CHARGE_LIMIT>]
          Get or set max charge limit
      --get-gpio [<GET_GPIO>]
          Get GPIO value by name or all, if no name provided
      --fp-led-level [<FP_LED_LEVEL>]
          Get or set fingerprint LED brightness level [possible values: high, medium, low, ultra-low, auto]
      --fp-brightness [<FP_BRIGHTNESS>]
          Get or set fingerprint LED brightness percentage
      --kblight [<KBLIGHT>]         Set keyboard backlight percentage or get, if no value provided
      --tablet-mode <TABLET_MODE>   Set tablet mode override [possible values: auto, tablet, laptop]
      --touchscreen-enable <TOUCHSCREEN_ENABLE>
          Enable/disable touchscreen [possible values: true, false]
      --console <CONSOLE>           Get EC console, choose whether recent or to follow the output [possible values: recent, follow]
      --reboot-ec <REBOOT_EC>       Control EC RO/RW jump [possible values: reboot, jump-ro, jump-rw, cancel-jump, disable-jump]
      --hash <HASH>                 Hash a file of arbitrary data
      --driver <DRIVER>             Select which driver is used. By default portio is used [possible values: portio, cros-ec, windows]
      --pd-addrs <PD_ADDRS> <PD_ADDRS>
          Specify I2C addresses of the PD chips (Advanced)
      --pd-ports <PD_PORTS> <PD_PORTS>
          Specify I2C ports of the PD chips (Advanced)
  -t, --test                        Run self-test to check if interaction with EC is possible
  -h, --help                        Print help information
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

###### Running on Windows

On newly released systems since 2025 the Framework driver installer includes the EC driver.
This includes Framework 12, Framework 13 AMD Ryzen AI 300, Framework Desktop.

Previous platforms will be enabled next.

Installing: `winget install FrameworkComputer.framework_tool`

##### Running on ChromeOS

The application can run on ChromeOS but most commands rely on custom host
commands that we built into the EC firmware of non-Chromebook Framework laptops.
In theory you could add those patches to the Chromebook platform, build your
own EC firmware and flash it.

## Tests

- [x] Basic unit tests
- [x] Test parsing real binaries

## Version Check

Check if the firmware version is what you expect, returns exit code 0 on
succcess, 1 on failure.

```
# Check which devices it's available for
> ./framework_tool --device
  [possible values: bios, ec, pd0, pd1, rtm01, rtm23, ac-left, ac-right]

For more information try '--help'

# Successful compare
> ./framework_tool --device bios --compare-version 03.01
Target Version "03.01"
Comparing BIOS version "03.01"
Compared version:   0
> echo $?
0

# Failed compare
> ./framework_tool --device bios --compare-version 03.00
    Finished dev [unoptimized + debuginfo] target(s) in 0.05s
Target Version "03.00"
Comparing BIOS version "03.01"
Compared version:   1
Error: "Fail"

> echo $?
1
```

On UEFI Shell:

```
# Check if AC is attached on left side
Shell> fs0:framework_tool.efi --device ac-left --compare-version 1
Target Version "1"
Comparing AcLeft "1"
Comparison Result: 0
# It is
Shell> echo %lasterror%
0x0

# Check if AC is attached on right side
Shell> fs0:framework_tool.efi --device ac-right --compare-version 1
Target Version "1"
Comparing AcLeft "0"
Comparison Result: 1

# It is not
Shell> echo %lasterror%
0x1
```

## Debugging

To debug, increase the verbosity from the commandline with `-v`.
The verbosity levels are:

| Commandline | Level  |
|-------------|--------|
| `-q`        | No log |
| None        | Error  |
| `-v`        | Warn   |
| `-vv`       | Info   |
| `-vvv`      | Debug  |
| `-vvvv`     | Trace  |

For example it is useful to check which EC driver is used:

```
> framework_tool --kblight -vvv
[DEBUG] Chromium EC Driver: CrosEc
[DEBUG] send_command(command=0x22, ver=0, data_len=0)
Keyboard backlight: 0%

> framework_tool --driver portio --kblight -vvv
[DEBUG] Chromium EC Driver: Portio
[DEBUG] send_command(command=0x22, ver=0, data_len=0)
Keyboard backlight: 0%
```

## FreeBSD

```
sudo pkg install hidapi

# Build the library and tool
cargo build

# Running the tool
cargo run
```
