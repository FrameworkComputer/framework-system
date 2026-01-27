# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Framework System is a Rust library and CLI tool for interacting with Framework Computer hardware. It targets Linux, Windows, UEFI, and FreeBSD. The MSRV is 1.81.

## Development and Testing advice

Most commands must be run as root, try to run them with sudo, usually I have fingerprint sudo enabled, if that fails, ask me to run them and provide the output.

By default build in debug mode that's way faster than `--release` builds.
On every commit all builds, lints and tests must keep working.
We also must not break other platforms (Windows, Linux, FreeBSD, UEFI).

## Build Commands

```bash
# Build library and CLI tool (default members: framework_lib, framework_tool)
cargo build
cargo build --release

# Build with specific features
cargo build -p framework_lib --features nvidia

# Build UEFI application (separate target, not in default workspace members)
cd framework_uefi && make

# Run tests (only framework_lib has tests)
cargo test -p framework_lib

# Linting (CI enforces -D warnings)
cargo clippy -- -D warnings
cargo fmt --check

# Generate docs (CI enforces -D warnings)
RUSTDOCFLAGS="-Dwarnings" cargo doc

# Generate shell completions (must stay in sync; CI checks this)
cargo run -- --completions bash > completions/bash/framework_tool
cargo run -- --completions zsh > completions/zsh/_framework_tool
cargo run -- --completions fish > completions/fish/framework_tool.fish
```

## Architecture

### Workspace Structure

- **framework_lib/** — Core library. Contains all hardware interaction logic, command parsing, and platform abstractions. Supports `no_std` (for UEFI) via the `uefi` feature flag.
- **framework_tool/** — Thin CLI binary wrapping `framework_lib::commandline`. Entry point is ~55 lines.
- **framework_uefi/** — UEFI shell application. Built separately via Makefile (not a default workspace member). Uses `no_std` with `alloc`.

### Key Modules in framework_lib

- `chromium_ec/` — Chrome EC controller communication. Multiple driver backends: `cros_ec` (Linux kernel), `portio` (direct I/O for UEFI/FreeBSD), `windows` (Windows driver). Commands defined in `commands.rs`.
- `ccgx/` — USB Power Delivery controller (CCG5/CCG6/CCG8) firmware parsing and device management. `binary.rs` handles firmware binary parsing.
- `commandline/` — CLI implementation. `clap_std.rs` for std platforms (uses clap), `uefi.rs` for UEFI-specific parsing.
- `smbios.rs` — SMBIOS table parsing for hardware identification.
- `power.rs` — Battery, AC adapter, and PD port information.
- `csme.rs` — Intel CSME/ME firmware info parsing.
- `util.rs` — Platform detection and identification. `Platform` enum identifies specific hardware models. Platform detection is cached globally via `lazy_static`.

### Platform Abstraction Patterns

- **OS-level:** `#[cfg(target_os = "linux")]`, `#[cfg(windows)]`, `#[cfg(target_os = "freebsd")]`
- **Feature-level:** `#[cfg(feature = "rusb")]`, `#[cfg(feature = "hidapi")]`, `#[cfg(feature = "uefi")]`
- **no_std compatibility:** UEFI builds use `#![no_std]` with `alloc`. `lazy_static` uses `spin::Mutex` for no_std, `std::sync::Mutex` for std. Custom `no_std_compat` wrapper bridges standard library types.

### Feature Flags (framework_lib)

- `default` — Enables `hidapi` and `rusb`
- `readonly` — Disables hardware modification commands
- `uefi` — UEFI build support (no_std)
- `hidapi` — HID device access (touchpad, touchscreen, PD controllers)
- `rusb` — USB device access (audio card, camera, input modules)
- `nvidia` — NVIDIA GPU monitoring

### Custom Dependency Forks

The project patches `uefi`, `uefi-services`, and uses custom forks of `smbios-lib` and `rust-hwio` for no_std/FreeBSD support. See `[patch.crates-io]` in the root Cargo.toml.

## CI Pipeline

Runs on every push: Linux build, Windows build, UEFI build, FreeBSD build, tests, lints (clippy + fmt), doc generation. CI also verifies shell completions are up-to-date and that no untracked changes are introduced by the build.

## Test Binaries

`framework_lib/test_bins/` contains firmware binary dumps and SMBIOS dumps used for unit tests. Tests parse these files to validate binary parsing logic.
