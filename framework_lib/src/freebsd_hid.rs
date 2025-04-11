use nix::{ioctl_read, ioctl_read_buf, ioctl_readwrite, ioctl_readwrite_buf, ioctl_write_buf};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;

#[repr(C)]
pub struct HidIocGrInfo {
    pub bustype: u32,
    pub vendor: u16,
    pub product: u16,
}

//ioctl_readwrite!(hidraw_get_report_desc, b'U', 21, HidrawGetReportDesc);
//ioctl_readwrite!(hidraw_get_report, b'U', 23, HidrawGetReport);
//ioctl_write!(hidraw_set_report, b'U', 24, HidrawSetReport);
ioctl_read!(hidiocgrawninfo, b'U', 32, HidIocGrInfo);
//ioctl_readwrite!(hidiocgrawnname, b'U', 33, HidIocGrName);
ioctl_read_buf!(hid_raw_name, b'U', 33, u8);
ioctl_write_buf!(hid_set_feature, b'U', 35, u8);
ioctl_readwrite_buf!(hid_get_feature, b'U', 36, u8);

pub fn hidraw_open(vid: u16, pid: u16) -> Option<std::fs::File> {
    // TODO: List files in the directory
    for i in 0..32 {
        let path = format!("/dev/hidraw{}", i);
        let file = if let Ok(f) = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&path)
        {
            f
        } else {
            debug!("{} not found", path);
            continue;
        };

        let mut desc = HidIocGrInfo {
            bustype: 0,
            vendor: 0,
            product: 0,
        };
        unsafe {
            let fd = file.as_raw_fd();
            if let Err(err) = hidiocgrawninfo(fd, &mut desc) {
                error!("Failed to access hidraw at {}: {:?}", path, err);
                return None;
            }
            debug!(
                "Found {:04X}:{:04X} Bustype: {:04X}",
                desc.vendor, desc.product, desc.bustype
            );
            if desc.vendor == vid && desc.product == pid {
                return Some(file);
            }
        }
    }
    error!("No matching hidraw found. Is the hidraw kernel module loaded?");
    None
}
