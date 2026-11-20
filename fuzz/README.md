# Fuzzing

Fuzz the parsing of EC responses (memory map and host commands) without hardware,
and the parsing of firmware binaries.
With `--cfg fuzzing` (set by cargo-fuzz), `CrosEc` uses a mock driver
(`framework_lib/src/chromium_ec/fuzz_driver.rs`) that returns fuzzer-provided data.

Input layout: `[platform: u8][memmap: 255 bytes][host command responses...]`

Each host command response starts with a control byte:

- `0x00..=0xBF`: Success, response of the size the caller expects
- `0xC0..=0xDF`: Success, length from the next byte
- `0xE0..=0xFF`: EC error status

Once the input is exhausted, all host commands fail.

## Running

Requires nightly and `cargo install cargo-fuzz`.

```sh
cd fuzz
cargo +nightly fuzz run ec_memmap -- -close_fd_mask=1 -timeout=5
cargo +nightly fuzz run ec_host_cmd -- -close_fd_mask=1 -timeout=5

# Keep going after crashes and collect all of them
cargo +nightly fuzz run ec_host_cmd -- -fork=4 -ignore_crashes=1 -close_fd_mask=1

# Reproduce a crash
cargo +nightly fuzz run ec_memmap artifacts/ec_memmap/crash-...
```

`-close_fd_mask=1` silences stdout, the library prints a lot.

## Firmware binary parsing

These targets take the firmware file itself as input, so crash artifacts can be
reproduced with the real tool (`framework_tool --ec-bin`/`--pd-bin`).

- `ec_binary`: EC firmware binaries
- `ccgx_binary`: CCGX PD firmware binaries, tried with every silicon family
- `capsule`: UEFI capsules, including the Windows UX capsule header (`--capsule`)
- `h2o_capsule`: Finding BIOS version, EC and PD firmware in Insyde BIOS capsules (`--h2o-capsule`)

The interesting data is at offsets far beyond libFuzzer's default max input
length, so seed the corpus with real binaries and raise `-max_len`:

```sh
cd fuzz
mkdir -p corpus/ec_binary corpus/ccgx_binary
cp ../framework_lib/test_bins/*-ec-*.bin corpus/ec_binary/
cp ../framework_lib/test_bins/*pd*.bin ../framework_lib/test_bins/dahlia-*.bin corpus/ccgx_binary/

cargo +nightly fuzz run ec_binary -- -close_fd_mask=1 -max_len=524288
cargo +nightly fuzz run ccgx_binary -- -close_fd_mask=1 -max_len=262144

mkdir -p corpus/capsule && cp ../framework_lib/test_bins/winux.bin corpus/capsule/
cargo +nightly fuzz run capsule -- -close_fd_mask=1 -max_len=1048576
cargo +nightly fuzz run h2o_capsule -- -close_fd_mask=1 -max_len=1048576
```
