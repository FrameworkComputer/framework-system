#!/usr/bin/env python3
"""Generate seeds for the h2o_capsule fuzz target from test_bins

Real BIOS capsules are too big to check in, so build minimal ones that
contain just the markers and the embedded EC and PD firmware.
"""
import itertools
import pathlib

BINS = pathlib.Path(__file__).parent / "../framework_lib/test_bins"
OUT = pathlib.Path(__file__).parent / "corpus/h2o_capsule"

# PD region sizes in the capsule, see capsule_content.rs
PD_LEN = {b"\x00\x20": 0x20000, b"\x00\x40": 0x20000, b"\x00\x80": 0x40000}

OUT.mkdir(parents=True, exist_ok=True)
ecs = sorted(BINS.glob("*-ec-*.bin"))
pds = [p for p in sorted(BINS.glob("*-pd-*.bin")) if p.read_bytes()[:2] in PD_LEN]
for ec, pd in itertools.product(ecs, pds):
    pd_data = pd.read_bytes()
    pd_data = pd_data.ljust(PD_LEN[pd_data[:2]], b"\xff")
    seed = (
        b"$BVDT" + b"\x00" * 9 + b"HFW30" + b"\x00" + b"03.05" + b"\x00" * 8
        + b"$_RETIMER_PARAM_" + b"\x00" * 8 + b"\x34\x12"
        + b"$_IFLASH_EC_IMG_" + b"\x00" * 8 + ec.read_bytes()
        + pd_data
    )
    (OUT / f"{ec.stem}+{pd.stem}.bin").write_bytes(seed)
    print(OUT / f"{ec.stem}+{pd.stem}.bin")
