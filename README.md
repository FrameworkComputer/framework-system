# Framework System

Rust libraries and tools to interact with the system.

The tool works on Linux, Windows and the UEFI shell.
Most features are supported on every "OS".

You can find lots of examples in [EXAMPLES.md](./EXAMPLES.md).

## Installation

### Linux

- [NixOS](https://github.com/NixOS/nixpkgs/blob/nixos-25.05/pkgs/by-name/fr/framework-tool/package.nix)
  - `nix-shell -p framework-tool`
- [ArchLinux](https://archlinux.org/packages/extra/x86_64/framework-system/)
  - `pacman -Sy framework-system`
- [Bazzite](https://github.com/ublue-os/bazzite/pull/3026)
- Others
  - Build from source
  - Or download [latest binary](https://github.com/FrameworkComputer/framework-system/releases/latest/download/framework_tool)
- ChromeOS
  - Build from source

### Windows

```
winget install FrameworkComputer.framework_tool
```

### FreeBSD

```
sudo pkg install framework-system
```

## Features

To check which features are supported on which OS and platform,
see the [Support Matrices](support-matrices.md).

###### Operating System Support

The following operating environments are supported.

- Linux
- Windows
- UEFI
- FreeBSD

Most functionality depends communication with the EC.
For Linux and Windows there are dedicated drivers.
On UEFI and FreeBSD raw port I/O is used - on Linux this can also be used as a fallback, if the driver is not available or not working.

|                     | Port I/O | Linux | Windows |
|---------------------|----------| ------|---------|
| Framework 12        |          |       |         |
| Intel Core 12th Gen | Yes      | [6.12](https://github.com/torvalds/linux/commit/62be134abf4250474a7a694837064bc783d2b291) | Yes        |
| Framework 13        |          |       |         |
| Intel Core 11th Gen | Yes      | [6.11](https://github.com/torvalds/linux/commit/04ca0a51f1e63bd553fd4af8e9af0fe094fa4f0a) | Not yet    |
| Intel Core 12th Gen | Yes      | [6.13](https://github.com/torvalds/linux/commit/dcd59d0d7d51b2a4b768fc132b0d74a97dfd6d6a) | Not yet    |
| Intel Core 13th Gen | Yes      | [6.13](https://github.com/torvalds/linux/commit/dcd59d0d7d51b2a4b768fc132b0d74a97dfd6d6a) | Not yet    |
| AMD Ryzen 7040      | Yes      | [6.10](https://github.com/torvalds/linux/commit/c8f460d991df93d87de01a96b783cad5a2da9616) | BIOS 3.16+ |
| Intel Core Ultra 1S | Yes      | [6.12](https://github.com/torvalds/linux/commit/62be134abf4250474a7a694837064bc783d2b291) | Soon       |
| AMD Ryzen AI 300    | Yes      | [6.12](https://github.com/torvalds/linux/commit/62be134abf4250474a7a694837064bc783d2b291) | Yes        |
| Framework 16        |          |       |         |
| AMD Ryzen 7040      | Yes      | [6.10](https://github.com/torvalds/linux/commit/c8f460d991df93d87de01a96b783cad5a2da9616) | BIOS 3.06+ |
| Framework Desktop   |          |       |         |
| AMD Ryzen AI Max    | Yes      | [6.15](https://github.com/torvalds/linux/commit/d83c45aeec9b223fe6db4175e9d1c4f5699cc37a) | Yes        |

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

# FreeBSD
sudo pkg install hidapi
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

##### Running on ChromeOS

The application can run on ChromeOS but most commands rely on custom host
commands that we built into the EC firmware of non-Chromebook Framework laptops.
In theory you could add those patches to the Chromebook platform, build your
own EC firmware and flash it.

## Tests

- [x] Basic unit tests
- [x] Test parsing real binaries
