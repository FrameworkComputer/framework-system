# Framework System

Rust libraries and tools to interact with the system.

The tool works on Linux, Windows and the UEFI shell.
Most features are supported on every "OS".

The workspace also contains an optional .NET interop crate, `framework_dotnet_ffi`,
which exposes a native FFI surface over `framework_lib` and generates C# P/Invoke
bindings via `csbindgen`.

You can find lots of examples in [EXAMPLES.md](./EXAMPLES.md).

## Installation

### Linux

- NixOS/nixpkgs [![nixpkgs unstable package](https://repology.org/badge/version-for-repo/nix_unstable/framework-tool.svg)](https://repology.org/project/framework-tool/versions)
  - `nix-shell -p framework-tool`
- [ArchLinux](https://archlinux.org/packages/extra/x86_64/framework-system/) [![Arch Linux package](https://repology.org/badge/version-for-repo/arch/framework-system.svg)](https://repology.org/project/framework-system/versions)
  - `pacman -S framework-system`
- [Bazzite](https://github.com/ublue-os/bazzite/pull/3026)
  - No need to install - included in the base image
- [Fedora (via the Terra repo)](https://github.com/terrapkg/packages/pull/8554)
  - Install the [Terra repository](https://terra.fyralabs.com/), then `dnf install framework-system`
- OpenSUSE [![openSUSE Tumbleweed package](https://repology.org/badge/version-for-repo/opensuse_tumbleweed/framework-tool.svg)](https://repology.org/project/framework-tool/versions)
  - `zypper install framework_tool`
- Gentoo [![Gentoo package](https://repology.org/badge/version-for-repo/gentoo/framework-tool.svg)](https://repology.org/project/framework-tool/versions)
  - `emerge framework_tool`
- Debian 14 (Forky) and later ([rust-framework-lib](https://tracker.debian.org/pkg/rust-framework-lib), [rust-framework-tool](https://tracker.debian.org/pkg/rust-framework-tool))
  - `apt install rust-framework-tool`
- Ubuntu 26.04 (Resolute) and later ([rust-framework-lib](https://launchpad.net/ubuntu/+source/rust-framework-lib), [rust-framework-tool](https://launchpad.net/ubuntu/+source/rust-framework-tool))
  - `apt install rust-framework-tool`
- Ubuntu (Any Version - [Snap Package](https://snapcraft.io/framework-tool))
  - `snap install --edge framework-tool --classic`
  - Note that this installs the command called `framework-tool` instead of `framework_tool`
- Rhino Linux [![Rhino Linux package](https://repology.org/badge/version-for-repo/pacstall/framework-system.svg)](https://repology.org/project/framework-system/versions)
  - `pacstall -I framework-system-bin`
- [![Void Linux x86_64 package](https://repology.org/badge/version-for-repo/void_x86_64/framework-system.svg)](https://repology.org/project/framework-system/versions)
  - `sudo xbps-install -S framework-system`
- Manjaro
  - [![Manjaro Stable package](https://repology.org/badge/version-for-repo/manjaro_stable/framework-system.svg)](https://repology.org/project/framework-system/versions)
  - [![Manjaro Testing package](https://repology.org/badge/version-for-repo/manjaro_testing/framework-system.svg)](https://repology.org/project/framework-system/versions)
  - [![Manjaro Unstable package](https://repology.org/badge/version-for-repo/manjaro_unstable/framework-system.svg)](https://repology.org/project/framework-system/versions)
  - `pacman -S framework-system`
- Others
  - Build from source
  - Or download [latest binary](https://github.com/FrameworkComputer/framework-system/releases/latest/download/framework_tool)
- ChromeOS
  - Build from source
- Homebrew
  - Add [this homebrew tap](https://github.com/ublue-os/homebrew-tap), then `brew install --cask framework-tool`
- Cargo (Any distro)
  - `cargo install --locked framework_tool`

### Windows

```
winget install framework_tool --source winget
```

### FreeBSD

[![FreeBSD port](https://repology.org/badge/version-for-repo/freebsd/framework-system.svg)](https://repology.org/project/framework-system/versions)

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
| Framework Laptop 12 |          |       |         |
| Intel Core 12th Gen | Yes      | [6.12](https://github.com/torvalds/linux/commit/62be134abf4250474a7a694837064bc783d2b291) | Yes        |
| Framework Laptop 13 |          |       |         |
| Intel Core 11th Gen | Yes      | [6.11](https://github.com/torvalds/linux/commit/04ca0a51f1e63bd553fd4af8e9af0fe094fa4f0a) | Not yet    |
| Intel Core 12th Gen | Yes      | [6.13](https://github.com/torvalds/linux/commit/dcd59d0d7d51b2a4b768fc132b0d74a97dfd6d6a) | Not yet    |
| Intel Core 13th Gen | Yes      | [6.13](https://github.com/torvalds/linux/commit/dcd59d0d7d51b2a4b768fc132b0d74a97dfd6d6a) | Not yet    |
| AMD Ryzen 7040      | Yes      | [6.10](https://github.com/torvalds/linux/commit/c8f460d991df93d87de01a96b783cad5a2da9616) | BIOS 3.16+ |
| Intel Core Ultra S1 | Yes      | [6.12](https://github.com/torvalds/linux/commit/62be134abf4250474a7a694837064bc783d2b291) | BIOS 3.06+ |
| AMD Ryzen AI 300    | Yes      | [6.12](https://github.com/torvalds/linux/commit/62be134abf4250474a7a694837064bc783d2b291) | Yes        |
| Framework Laptop 16 |          |       |         |
| AMD Ryzen 7040      | Yes      | [6.10](https://github.com/torvalds/linux/commit/c8f460d991df93d87de01a96b783cad5a2da9616) | BIOS 3.06+ |
| AMD Ryzen AI 300    | Yes      | [6.10](https://github.com/torvalds/linux/commit/c8f460d991df93d87de01a96b783cad5a2da9616) | Yes        |
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
- [x] Get information about USB-C PD ports (`--pdports`)
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
- [x] Framework Laptop 16 (AMD Ryzen AI 300)
- [x] Framework Desktop (AMD Ryzen AI Max 300)
- [x] Port I/O communication on Linux
- [x] Port I/O communication in UEFI
- [x] Port I/O communication on FreeBSD
- [x] Using `cros_ec` driver in Linux kernel
- [x] Using [Framework EC Windows driver](https://github.com/FrameworkComputer/crosecbus) based on [coolstar's](https://github.com/coolstar/crosecbus)
- [x] Using [DHowett's Windows CrosEC driver](https://github.com/DHowett/FrameworkWindowsUtils)

## Building

### Dependencies

[Rustup](https://rustup.rs/) is convenient for setting up the right Rust version.
Based on `rust-toolchain.toml` it will install the right toolchain and version for this project.

MSRV (Minimum Supported Rust Version):

- 1.74 for Linux/Windows
- 1.74 for UEFI

System dependencies

```
# NixOS
nix-shell --run fish -p cargo systemd udev hidapi pkg-config
# NixOS using flakes
nix develop
# Or build fully using flake, see section below

# Fedora
sudo dnf install systemd-devel hidapi-devel

# FreeBSD
sudo pkg install rust hidapi pkgconf

# Debian/Ubuntu
sudo apt install rustup build-essential libhidapi-dev libsystemd-dev libudev-dev libusb-1.0-0-dev pkg-config
```

### Nix Flake

If you have Nix with flakes enabled, you can build and develop without manually installing dependencies.

Run directly from GitHub
```sh
# Run directly from GitHub
nix run github.com:FrameworkComputer/framework-system -- --versions
```

Build and run locally after cloning:

```
# Build the CLI tool (release)
nix build .#tool

# Build the CLI tool (debug)
nix build .#tool-debug

# Build the UEFI application (release)
nix build .#uefi

# Run the CLI tool directly
nix run .#tool -- --help

# Cross-compile for Windows (from Linux, no Windows needed)
nix build .#windows

# Run the UEFI app in QEMU
nix run .#qemu

# Enter a development shell with all dependencies
nix develop

# Enter a cross-compilation shell for Windows
nix develop .#cross-windows
cargo build --target x86_64-pc-windows-gnu -p framework_tool
```

### Building with Cargo

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

# Building the optional .NET interop crate
cargo build -p framework_dotnet_ffi

# Build the UEFI application
# Can't be built with cargo! That's why we need to exclude it in the other commands.
make -C framework_uefi
ls -l framework_uefi/build/x86_64-unknown-uefi/boot.efi
```

### Optional .NET Interop

The `framework_dotnet_ffi` crate is an optional workspace member and is not part of
the default `cargo build` / `cargo check` set.

Build it explicitly when you want to consume Framework functionality from .NET:

```sh
cargo build -p framework_dotnet_ffi
```

Building the crate also regenerates the low-level C# bindings at
`framework_dotnet_ffi/csharp/NativeMethods.g.cs` using `csbindgen`.

- Native library name: `framework_dotnet_ffi`
- Generated C# namespace: `Framework.System.Interop`
- Generated C# class: `NativeMethods`

## Install local package

```
> cargo install --path framework_tool
> which framework_tool
/home/zoid/.cargo/bin/framework_tool
```

## Running

Run without any arguments to see the help.

Many actions require root. First build with cargo and then run the binary with sudo:

```sh
cargo build && sudo ./target/debug/framework_tool
```
