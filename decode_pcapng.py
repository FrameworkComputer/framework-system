#!/usr/bin/env python3

from pcapng import FileScanner, blocks
import struct
from collections import namedtuple
import sys

# From https://github.com/JohnDMcMaster/usbrply/blob/master/usbrply/win_pcap.py#L171
usb_urb_win_nt = namedtuple(
    'usb_urb_win',
    (
        # Length of entire packet entry including htis header and additional pkt_len data
        'pcap_hdr_len',
        # IRP ID
        # buffer ID or something like that
        # it is not a unique packet ID
        # but can be used to match up submit and response
        'id',
        # IRP_USBD_STATUS
        'irp_status',
        # USB Function
        'usb_func',
        # IRP Information
        # Ex: Direction: PDO => FDO
        'irp_info',
        # USB port
        # Ex: 3
        'bus_id',
        # USB device on that port
        # Ex: 16
        'device',
        # Which endpoint on that bus
        # Ex: 0x80 (0 in)
        'endpoint',
        # Ex: URB_CONTROL
        'transfer_type',
        # Length of data beyond header
        'data_length',
    ))
usb_urb_win_fmt = (
    '<'
    'H'  # pcap_hdr_len
    'Q'  # irp_id
    'i'  # irp_status
    'H'  # usb_func
    'B'  # irp_info
    'H'  # bus_id
    'H'  # device
    'B'  # endpoint
    'B'  # transfer_type
    'I'  # data_length
)
usb_urb_sz = struct.calcsize(usb_urb_win_fmt)
def usb_urb(s):
    return usb_urb_win_nt(*struct.unpack(usb_urb_win_fmt, bytes(s)))

# transfer_type=2 is URB_CONTROL
# irp_info: 0 means from host, 1 means from device

# To find them, look at the pcap in wireguard and check the source/destination
images = {
    101: {
        'type': 'DP',
        'filename': 'reflash101.pcapng',
        'second_first': False,
        'devices': {
            1: {
                'busid': 2,
                'device': 12,
            },
            2: {
                'busid': 2,
                'device': 13,
            }
        }
    },
    8: {
        'type': 'DP',
        'filename': 'flash-100-to-8.pcapng',
        'second_first': False,
        'devices': {
            1: {
                'busid': 1,
                'device': 8,
            },
            2: {
                'busid': 1,
                'device': 9,
            }
        }
    },
    100: {
        'type': 'DP',
        'filename': 'reflash100.pcapng',
        'second_first': False,
        'devices': {
            1: {
                'busid': 1,
                'device': 6,
            },
            2: {
                'busid': 1,
                'device': 7,
            }
        }
    },
    # HDMI
    6: {
        'type': 'HDMI',
        # Yeah.. wrong filename
        'filename': 'hdmi-flash-5.pcapng',
        'second_first': False,
        'devices': {
            1: {
                'busid': 1,
                'device': 29,
            },
            2: {
                'busid': 1,
                'device': 30,
            }
        }
    },
    105: {
        'type': 'HDMI',
        'filename': 'hdmi-flash-105.pcapng',
        'second_first': False,
        'devices': {
            1: {
                'busid': 1,
                'device': 26,
            },
            2: {
                'busid': 1,
                'device': 27,
            }
        }
    },
}

ROW_SIZE = 128
MAX_ROWS = 1024
FW_VERSION = None

DEBUG = False
VERBOSE = False


def format_hex(buf):
    return ''.join('{:02x} '.format(x) for x in buf)

def print_image_info(binary, index):
    rows = len(binary)
    size = rows * len(binary[0][1])
    print("Image {} Size:    {} B, {} rows".format(index, size, rows))
    print("  FW at: 0x{:04X} Metadata at 0x{:04X}".format(binary[0][0], binary[-1][0]))

def twos_comp(val, bits):
    """compute the 2's complement of int value val"""
    if (val & (1 << (bits - 1))) != 0: # if sign bit is set e.g., 8bit: 128-255
        val = val - (1 << bits)        # compute negative value
    return val                         # return positive value as isdef twos_comp(val, bits):
    """compute the 2's complement of int value val"""
    if (val & (1 << (bits - 1))) != 0: # if sign bit is set e.g., 8bit: 128-255
        val = val - (1 << bits)        # compute negative value
    return val                         # return positive value as isi

def checksum_calc(s):
    sum = 0
    for c in s:
        sum = (sum + c) & 0xFF
    sum = -(sum % 256)
    return (sum & 0xFF)

def write_cyacd_row(f, row_no, data):
    # No idea what array ID is but it seems fine to keep it 0. Official builds also have that
    array_id = 0
    data_len = len(data)

    # Sum all bytes and calc two's complement
    cs_bytes = bytes([array_id, row_no&0xFF, (row_no&0xFF00)>>8, data_len&0xFF, (data_len&0xFF00)>>8])
    checksum = checksum_calc(cs_bytes+data)

    if data_len != 0x80:
        print("Len is {} instead of 0x80", data_len)
        sys.exit(1)
    data_hex = ''.join("{:02X}".format(x) for x in data)

    f.write(":{:02X}{:04X}{:04X}{}{:02X}\n".format(array_id, row_no, data_len, data_hex, checksum))

# Write the binary as cyacd file. Can only hold one firmware image per file
def write_cyacd(path, binary1):
    with open(path, "w") as f:
        # CYACD Header
        # Si ID  ########
        # Si Rev         ##
        # Checksum Type    ##
        f.write("1D0011AD0000\n")

        for (addr, row) in binary1[0:-1]:
            write_cyacd_row(f, addr, row)

        write_cyacd_row(f, binary1[-1][0], binary1[-1][1])


# Write the binary in the same layout with padding as on flash
def write_bin(path, binary1, binary2):
    with open(path, "wb") as f:
        # Write fist padding
        # Verified
        print("Padding  1: {:04X} to {:04X}".format(0, binary1[0][0]))
        for row in range(0, binary1[0][0]):
            f.write(b'\0' * ROW_SIZE)

        # Verified
        print("FW IMG   1: {:04X} to {:04X}".format(binary1[0][0], binary1[-2][0]))
        for (addr, row) in binary1[0:-1]:
            f.write(row)

        print("Padding  2: {:04X} to {:04X}".format(binary1[-2][0], binary2[0][0]-1))
        for row in range(binary1[-2][0], binary2[0][0]-1):
            f.write(b'\0' * ROW_SIZE)

        print("FW IMG   2: {:04X} to {:04X}".format(binary2[0][0], binary2[-2][0]))
        for (addr, row) in binary2[0:-1]:
            f.write(row)

        print("Padding  3: {:04X} to {:04X}".format(binary2[-2][0], binary2[-1][0]-1))
        for row in range(binary2[-2][0], binary2[-1][0]-1):
            f.write(b'\0' * ROW_SIZE)

        # Notice that these are in reverse order!
        # FW2 metadata is before FW1 metadata
        print("Metadata 2: {:04X} to {:04X}".format(binary2[-1][0], binary2[-1][0]))
        f.write(binary2[-1][1])
        print("Metadata 1: {:04X} to {:04X}".format(binary1[-1][0], binary1[-1][0]))
        f.write(binary1[-1][1])

        # Pad from end of FW1 metadata
        print("Padding  4: {:04X} to {:04X}".format(binary1[-1][0], MAX_ROWS-1))
        for row in range(binary1[-1][0], MAX_ROWS-1):
            f.write(b'\0' * ROW_SIZE)


def check_assumptions(img1_binary, img2_binary):
    # TODO: Check that addresses are in order
    # TODO: Metadata right after each other

    # Check assumptions that the updater relies on
    if len(img1_binary) != len(img2_binary):
        print("VIOLATED Assumption that both images are of the same size!")
        sys.exit(1)
    if len(img1_binary[0][1]) != ROW_SIZE:
        print("VIOLATED Assumption that the row size is {} bytes! Is: {}", ROW_SIZE, img1_binary[0][1])
        sys.exit(1)
    if img1_binary[0][0] != 0x0030:
        print("VIOLATED Assumption that start row of image 1 is at 0x0030. Is at 0x{:04X}".format(img1_binary[0][0]))
        sys.exit(1)
    if img1_binary[-1][0] != 0x03FF:
        print("VIOLATED Assumption that metadata row of image 1 is at 0x03FF. Is at 0x{:04X}".format(img1_binary[-1][0]))
        sys.exit(1)
    if img2_binary[0][0] != 0x0200:
        print("VIOLATED Assumption that start row of image 2 is at 0x0200. Is at 0x{:04X}".format(img2_binary[0]))
        sys.exit(1)
    if img2_binary[-1][0] != 0x03FE:
        print("VIOLATED Assumption that metadata row of image 2 is at 0x03FE. Is at 0x{:04X}".format(img2_binary[-1][0]))
        sys.exit(1)
    if img1_binary == img2_binary:
        print("VIOLATED Assumption that both images are not the same");
        sys.exit(1)


def decode_pcapng(path, info):
    img1_binary = [] # [(addr, row)]
    img2_binary = [] # [(addr, row)]
    with open(path, "rb") as f:
        scanner = FileScanner(f)
        block_no = 1
        for i, block in enumerate(scanner):
            if type(block) is blocks.EnhancedPacket or type(block) is blocks.SimplePacket:
                img1 = False
                img2 = False

                #print(block_no, block.packet_len, block.packet_data)
                packet = block.packet_data
                urb = usb_urb(packet[0:usb_urb_sz])

                # Filter device
                if urb.bus_id == info['devices'][1]['busid'] and urb.device == info['devices'][1]['device']:
                    img1 = True
                elif urb.bus_id == info['devices'][2]['busid'] and urb.device == info['devices'][2]['device']:
                    img2 = True
                else:
                    continue

                # Only consider outgoing packets
                if urb.irp_info != 0:
                    continue

                #print(block_no, urb)

                # Skip small packets
                if urb.data_length != 140:
                    continue

                if DEBUG:
                    print(block_no, "  ", format_hex(packet))

                hid_packet = packet[36:]
                if DEBUG:
                    print(block_no, "  ", format_hex(hid_packet))

                addr = (hid_packet[3] << 8) + hid_packet[2]
                payload = hid_packet[4:]
                if VERBOSE:
                    print("{:4d} 0x{:08X} {}".format(block_no, addr, format_hex(payload)))

                if img1:
                    if info['second_first']:
                        img2_binary.append((addr, payload))
                    else:
                        img1_binary.append((addr, payload))
                elif img2:
                    if info['second_first']:
                        img1_binary.append((addr, payload))
                    else:
                        img2_binary.append((addr, payload))

                block_no += 1
            else:
                print(block)
    return (img1_binary, img2_binary)


def main():
    FW_VERSION = int(sys.argv[1])
    info = images[FW_VERSION]
    path = '/home/zoid/framework/dp-card-fw-update/{}'.format(info['filename'])

    (img1_binary, img2_binary) = decode_pcapng(path, info)

    check_assumptions(img1_binary, img2_binary)

    print("Firmware version: {}".format(FW_VERSION))

    print_image_info(img1_binary, 1)
    print_image_info(img2_binary, 2)

    # Write cyacd file, instead of binary because that's what FWUPD expects
    #write_bin("{}-{}.bin".format(info['type'], FW_VERSION), img1_binary, img2_binary)
    write_cyacd("{}-{}-1.cyacd".format(info['type'], FW_VERSION), img1_binary)
    write_cyacd("{}-{}-2.cyacd".format(info['type'], FW_VERSION), img2_binary)

if __name__ == "__main__":
    main()
