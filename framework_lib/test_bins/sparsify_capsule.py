#!/usr/bin/env python3
"""
Sparsify UEFI capsule files for efficient git storage.

This script zeroes out sections of capsule files that aren't needed for testing,
while preserving the file structure and all test-relevant data. The resulting
files compress extremely well with git's zlib compression (typically 99% reduction).

Preserved sections:
- Capsule header (first 64 bytes)
- $BVDT section (BIOS version info)
- $_IFLASH_EC_IMG_ section + EC binary
- All embedded PD firmware (CCG5, CCG6, CCG8)
- $_RETIMER_PARAM_ section (retimer version)

Usage:
    ./sparsify_capsule.py input.cap                    # Creates input.sparse.cap
    ./sparsify_capsule.py input.cap output.cap         # Specify output path
    ./sparsify_capsule.py *.cap                        # Process multiple files
"""

import argparse
import gzip
import os
import sys


def find_all(data: bytes, needle: bytes) -> list[int]:
    """Find all occurrences of needle in data."""
    results = []
    pos = 0
    while (found := data.find(needle, pos)) != -1:
        results.append(found)
        pos = found + len(needle)
    return results


def sparsify_capsule(data: bytes) -> tuple[bytearray, list[tuple[int, int, str]]]:
    """
    Zero out sections we don't need while preserving test-relevant data.

    Returns:
        Tuple of (sparsified data, list of preserved regions)
    """
    preserved = []

    # Always preserve capsule header (64 bytes to be safe)
    preserved.append((0, 64, "Capsule header"))

    # Find and preserve $BVDT section (BIOS version)
    bvdt = data.find(b'$BVDT')
    if bvdt != -1:
        preserved.append((bvdt, 128, "$BVDT (BIOS version)"))

    # Find and preserve $_IFLASH_EC_IMG_ + EC binary (128KB)
    ec_marker = data.find(b'$_IFLASH_EC_IMG_')
    if ec_marker != -1:
        # Marker + offset + EC binary (128KB)
        preserved.append((ec_marker, 16 + 9 + 131072, "$_IFLASH_EC_IMG_ + EC"))

    # Find and preserve all CCG8 PD binaries (~262KB each)
    CCG8_NEEDLE = bytes([0x00, 0x80, 0x00, 0x20, 0xAD, 0x0C])
    CCG8_SIZE = 262144
    for i, offset in enumerate(find_all(data, CCG8_NEEDLE), 1):
        preserved.append((offset, CCG8_SIZE, f"CCG8 PD {i}"))

    # Find and preserve all CCG6 PD binaries (~64KB each)
    CCG6_NEEDLE = bytes([0x00, 0x40, 0x00, 0x20, 0x11, 0x00])
    CCG6_SIZE = 65536
    for i, offset in enumerate(find_all(data, CCG6_NEEDLE), 1):
        preserved.append((offset, CCG6_SIZE, f"CCG6 PD {i}"))

    # Find and preserve all CCG5 PD binaries (~32KB each)
    CCG5_NEEDLE = bytes([0x00, 0x20, 0x00, 0x20, 0x11, 0x00])
    CCG5_SIZE = 32768
    for i, offset in enumerate(find_all(data, CCG5_NEEDLE), 1):
        preserved.append((offset, CCG5_SIZE, f"CCG5 PD {i}"))

    # Find and preserve $_RETIMER_PARAM_ section
    retimer = data.find(b'$_RETIMER_PARAM_')
    if retimer != -1:
        preserved.append((retimer, 64, "$_RETIMER_PARAM_"))

    # Sort by offset
    preserved.sort(key=lambda x: x[0])

    # Create zeroed version with only preserved regions
    result = bytearray(len(data))
    for offset, length, _ in preserved:
        end = min(offset + length, len(data))
        result[offset:end] = data[offset:end]

    return result, preserved


def process_file(input_path: str, output_path: str | None = None, verbose: bool = True) -> None:
    """Process a single capsule file."""
    if output_path is None:
        base, ext = os.path.splitext(input_path)
        output_path = f"{base}.sparse{ext}"

    # Read input
    with open(input_path, 'rb') as f:
        data = f.read()

    original_size = len(data)

    # Sparsify
    sparse_data, preserved = sparsify_capsule(data)

    # Write output
    with open(output_path, 'wb') as f:
        f.write(sparse_data)

    if verbose:
        # Calculate compression stats
        compressed = gzip.compress(bytes(sparse_data), compresslevel=9)

        print(f"{os.path.basename(input_path)}:")
        print(f"  Original size:     {original_size:>12,} bytes")
        print(f"  Preserved regions: {len(preserved)}")
        for offset, length, desc in preserved:
            print(f"    {offset:>10} - {offset+length:>10} ({length:>7} bytes): {desc}")
        print(f"  Compressed size:   {len(compressed):>12,} bytes ({100*len(compressed)/original_size:.2f}%)")
        print(f"  Output: {output_path}")
        print()


def main():
    parser = argparse.ArgumentParser(
        description="Sparsify UEFI capsule files for efficient git storage.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )
    parser.add_argument(
        'input', nargs='+',
        help='Input capsule file(s)'
    )
    parser.add_argument(
        '-o', '--output',
        help='Output path (only valid with single input file)'
    )
    parser.add_argument(
        '-q', '--quiet', action='store_true',
        help='Suppress verbose output'
    )

    args = parser.parse_args()

    if args.output and len(args.input) > 1:
        print("Error: --output can only be used with a single input file", file=sys.stderr)
        sys.exit(1)

    for input_path in args.input:
        if not os.path.exists(input_path):
            print(f"Error: {input_path} not found", file=sys.stderr)
            continue

        output_path = args.output if len(args.input) == 1 else None
        process_file(input_path, output_path, verbose=not args.quiet)


if __name__ == '__main__':
    main()
