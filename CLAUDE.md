# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Project Overview

Check the README.md for infos about the project and how to build.

If we're on NixOS we should build with `nix develop` shell for fast iteration.
We can also use `nix build` with various targets, for example to test windows build: `nix build .#windows`

## Development and Testing advice

Most commands must be run as root, try to run them with sudo, usually I have fingerprint sudo enabled, if that fails, ask me to run them and provide the output.

By default build in debug mode that's way faster than `--release` builds.
On every commit all builds, lints and tests must keep working.
We also must not break other platforms (Windows, Linux, FreeBSD, UEFI).
Shell completions should also be updated if we change the commandline, see completions/README.md
