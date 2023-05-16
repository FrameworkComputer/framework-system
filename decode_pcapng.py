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
    5: {
        'filename': 'hdmi-flash-5.pcapng',
        'second_first': True,
        'devices': {
            1: {
                'busid': 1,
                'device': 30,
            },
            2: {
                'busid': 1,
                'device': 29,
            }
        }
    },
    105: {
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

FW_VERSION = None

DEBUG = False
VERBOSE = True

def format_hex(buf):
    return ''.join('{:02x} '.format(x) for x in buf)

if __name__ == "__main__":
    img1_binary = b''
    img1_addresses = []
    img2_binary = b''
    img2_addresses = []
    FW_VERSION = int(sys.argv[1])
    info = images[FW_VERSION]
    with open('/home/zoid/framework/dp-card-fw-update/{}'.format(info['filename']), "rb") as f:
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
                        img2_addresses.append(addr)
                        img2_binary += payload
                    else:
                        img1_addresses.append(addr)
                        img1_binary += payload
                elif img2:
                    if info['second_first']:
                        img1_addresses.append(addr)
                        img1_binary += payload
                    else:
                        img2_addresses.append(addr)
                        img2_binary += payload

                block_no += 1
            else:
                pass
                #print(block)

    # Check assumptions that the updater relies on
    if len(img1_binary) != len(img2_binary):
        print("VIOLATED Assumption that both images are of the same size!")
        sys.exit(1)
    if len(img1_binary) % 128 != 0:
        print("VIOLATED Assumption that the row size is 128 bytes!")
        sys.exit(1)
    if img1_addresses[0] != 0x0030:
        print("VIOLATED Assumption that start row of image 1 is at 0x0030. Is at 0x{:04X}".format(img1_addresses[0]))
        sys.exit(1)
    if img1_addresses[-1] != 0x03FF:
        print("VIOLATED Assumption that metadata row of image 1 is at 0x03FF. Is at 0x{:04X}".format(img1_addresses[-1]))
        sys.exit(1)
    if img2_addresses[0] != 0x0200:
        print("VIOLATED Assumption that start row of image 2 is at 0x0200. Is at 0x{:04X}".format(img2_addresses[0]))
        sys.exit(1)
    if img2_addresses[-1] != 0x03FE:
        print("VIOLATED Assumption that metadata row of image 2 is at 0x03FE. Is at 0x{:04X}".format(img2_addresses[-1]))
        sys.exit(1)

    print("Firmware version: {}".format(FW_VERSION))

    with open("dump1.bin", "wb") as dump1:
        print("Image 1 Size:    {} B, {} rows".format(len(img1_binary), int(len(img1_binary)/128)))
        dump1.write(img1_binary)
    with open("dump2.bin", "wb") as dump2:
        print("Image 2 Size:    {} B, {} rows".format(len(img2_binary), int(len(img2_binary)/128)))
        dump2.write(img1_binary)
