// Inlined from https://github.com/FrameworkComputer/rust-hwio (freebsd branch)
// Original crate: redox_hwio by Jeremy Soller <jackpot51@gmail.com>
// SPDX-License-Identifier: MIT

#[cfg(all(
    not(target_os = "freebsd"),
    any(target_arch = "x86", target_arch = "x86_64")
))]
use core::arch::asm;

use core::marker::PhantomData;

/// Trait for hardware port I/O operations
pub trait Io {
    type Value: Copy
        + PartialEq
        + core::ops::BitAnd<Output = Self::Value>
        + core::ops::BitOr<Output = Self::Value>
        + core::ops::Not<Output = Self::Value>;

    fn read(&self) -> Self::Value;
    fn write(&mut self, value: Self::Value);
}

// ---- FreeBSD ioctl infrastructure ----

#[cfg(target_os = "freebsd")]
use nix::ioctl_readwrite;
#[cfg(target_os = "freebsd")]
use std::os::fd::AsRawFd;

#[cfg(target_os = "freebsd")]
#[repr(C)]
struct IoDevPioReq {
    access: u32,
    port: u32,
    width: u32,
    val: u32,
}

#[cfg(target_os = "freebsd")]
ioctl_readwrite!(iodev_rw, b'I', 0, IoDevPioReq);
#[cfg(target_os = "freebsd")]
const IODEV_PIO_READ: u32 = 0;
#[cfg(target_os = "freebsd")]
const IODEV_PIO_WRITE: u32 = 1;

#[cfg(target_os = "freebsd")]
use std::{
    fs::{File, OpenOptions},
    sync::Mutex,
};

#[cfg(target_os = "freebsd")]
lazy_static! {
    static ref FILE: Mutex<File> = Mutex::new(
        OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/io")
            .expect("failed to open /dev/io")
    );
}

#[cfg(target_os = "freebsd")]
#[inline(always)]
fn port_read(port: u16, buf: &mut [u8]) {
    let file = FILE.lock().unwrap();
    let fd = file.as_raw_fd();

    let mut req = IoDevPioReq {
        access: IODEV_PIO_READ,
        port: port as u32,
        width: buf.len() as u32,
        val: 0,
    };
    unsafe {
        iodev_rw(fd, &mut req).unwrap();
    }

    match buf.len() {
        1 => {
            buf[0] = req.val as u8;
        }
        2 => {
            let val = u16::to_le_bytes(req.val as u16);
            buf[0] = val[0];
            buf[1] = val[1];
        }
        _ => panic!("Unsupported port_read width"),
    }
}

#[cfg(target_os = "freebsd")]
#[inline(always)]
fn port_write(port: u16, buf: &[u8]) {
    let file = FILE.lock().unwrap();
    let fd = file.as_raw_fd();

    let val = match buf.len() {
        1 => buf[0] as u32,
        2 => u16::from_le_bytes([buf[0], buf[1]]) as u32,
        _ => panic!("Unsupported port_write width"),
    };

    let mut req = IoDevPioReq {
        access: IODEV_PIO_WRITE,
        port: port as u32,
        width: buf.len() as u32,
        val,
    };
    unsafe {
        iodev_rw(fd, &mut req).unwrap();
    }
}

// ---- Linux /dev/port fallback for non-x86 architectures ----

#[cfg(all(
    target_os = "linux",
    not(any(target_arch = "x86", target_arch = "x86_64"))
))]
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    sync::Mutex,
};

#[cfg(all(
    target_os = "linux",
    not(any(target_arch = "x86", target_arch = "x86_64"))
))]
lazy_static! {
    static ref FILE: Mutex<File> = Mutex::new(
        OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/port")
            .expect("failed to open /dev/port")
    );
}

#[cfg(all(
    target_os = "linux",
    not(any(target_arch = "x86", target_arch = "x86_64"))
))]
#[inline(always)]
fn port_read(port: u16, buf: &mut [u8]) {
    let mut file = FILE.lock().unwrap();
    file.seek(SeekFrom::Start(port as u64)).unwrap();
    file.read_exact(buf).unwrap();
}

#[cfg(all(
    target_os = "linux",
    not(any(target_arch = "x86", target_arch = "x86_64"))
))]
#[inline(always)]
fn port_write(port: u16, buf: &[u8]) {
    let mut file = FILE.lock().unwrap();
    file.seek(SeekFrom::Start(port as u64)).unwrap();
    file.write_all(buf).unwrap();
}

// ---- Pio struct ----

/// Port I/O
#[derive(Copy, Clone)]
pub struct Pio<T> {
    port: u16,
    value: PhantomData<T>,
}

impl<T> Pio<T> {
    /// Create a PIO from a given port
    pub const fn new(port: u16) -> Self {
        Pio::<T> {
            port,
            value: PhantomData,
        }
    }
}

// ---- Pio<u8> implementation ----

impl Io for Pio<u8> {
    type Value = u8;

    #[cfg(all(
        not(target_os = "freebsd"),
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    #[inline(always)]
    fn read(&self) -> u8 {
        let value: u8;
        unsafe {
            asm!("in al, dx", out("al") value, in("dx") self.port, options(nostack));
        }
        value
    }

    #[cfg(any(
        target_os = "freebsd",
        not(any(target_arch = "x86", target_arch = "x86_64"))
    ))]
    #[inline(always)]
    fn read(&self) -> u8 {
        let mut buf = [0];
        port_read(self.port, &mut buf);
        buf[0]
    }

    #[cfg(all(
        not(target_os = "freebsd"),
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    #[inline(always)]
    fn write(&mut self, value: u8) {
        unsafe {
            asm!("out dx, al", in("al") value, in("dx") self.port, options(nostack));
        }
    }

    #[cfg(any(
        target_os = "freebsd",
        not(any(target_arch = "x86", target_arch = "x86_64"))
    ))]
    #[inline(always)]
    fn write(&mut self, value: u8) {
        let buf = [value];
        port_write(self.port, &buf);
    }
}

// ---- Pio<u16> implementation ----

impl Io for Pio<u16> {
    type Value = u16;

    #[cfg(all(
        not(target_os = "freebsd"),
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    #[inline(always)]
    fn read(&self) -> u16 {
        let value: u16;
        unsafe {
            asm!("in ax, dx", out("ax") value, in("dx") self.port, options(nostack));
        }
        value
    }

    #[cfg(any(
        target_os = "freebsd",
        not(any(target_arch = "x86", target_arch = "x86_64"))
    ))]
    #[inline(always)]
    fn read(&self) -> u16 {
        let mut buf = [0, 0];
        port_read(self.port, &mut buf);
        buf[0] as u16 | (buf[1] as u16) << 8
    }

    #[cfg(all(
        not(target_os = "freebsd"),
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    #[inline(always)]
    fn write(&mut self, value: u16) {
        unsafe {
            asm!("out dx, ax", in("ax") value, in("dx") self.port, options(nostack));
        }
    }

    #[cfg(any(
        target_os = "freebsd",
        not(any(target_arch = "x86", target_arch = "x86_64"))
    ))]
    #[inline(always)]
    fn write(&mut self, value: u16) {
        let buf = [value as u8, (value >> 8) as u8];
        port_write(self.port, &buf);
    }
}
