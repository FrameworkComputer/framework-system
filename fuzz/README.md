# Fuzzing

Fuzz the parsing of EC responses (memory map and host commands) without hardware.
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
