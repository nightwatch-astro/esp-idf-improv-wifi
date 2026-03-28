//! ESP-IDF UART transport adapter.
//!
//! This module is only available when the `esp-idf-svc` feature is enabled.
//! It provides a UART transport that implements `std::io::Read + Write`
//! using ESP-IDF's VFS (Virtual File System) layer.
//!
//! # Example
//!
//! ```rust,no_run
//! use esp_idf_improv_wifi::esp_idf::UartTransport;
//!
//! let transport = UartTransport::open("/dev/uart/0").expect("open UART");
//! ```

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};

/// UART transport using ESP-IDF's VFS (POSIX file descriptor).
///
/// Opens a UART device file in read-write, non-blocking mode.
/// The UART must be configured externally (baud rate, pins, etc.)
/// before constructing this transport.
pub struct UartTransport {
    file: File,
}

impl UartTransport {
    /// Open a UART device via VFS path (e.g., "/dev/uart/0").
    ///
    /// The UART is opened in read-write mode. Non-blocking behavior
    /// depends on the underlying VFS driver configuration.
    pub fn open(path: &str) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        Ok(Self { file })
    }
}

impl Read for UartTransport {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
}

impl Write for UartTransport {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
