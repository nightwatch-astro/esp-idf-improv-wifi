//! ImprovWiFi serial provisioning protocol for esp-idf-svc.
//!
//! This crate implements the [Improv WiFi](https://www.improv-wifi.com/) serial
//! provisioning protocol, allowing IoT devices to receive WiFi credentials over
//! a serial (UART) connection from a browser or provisioning tool.
//!
//! # Architecture
//!
//! The core protocol logic is transport-agnostic — it operates on any
//! `std::io::Read + Write` stream. An optional `esp-idf-svc` feature flag
//! provides a UART transport adapter for ESP32.

pub mod packet;
mod protocol;
mod types;

#[cfg(feature = "esp-idf-svc")]
pub mod esp_idf;

// Re-export public API
pub use packet::ParseError;
pub use protocol::{ImprovWifi, ImprovWifiBuilder};
pub use types::{
    Command, DeviceInfo, ImprovError, ImprovState, PacketType, WifiCredentials, WifiNetwork,
};

/// Improv serial protocol version.
pub const PROTOCOL_VERSION: u8 = 1;

/// Packet header bytes: "IMPROV".
pub const HEADER: [u8; 6] = [0x49, 0x4D, 0x50, 0x52, 0x4F, 0x56];

/// Default UART baud rate for Improv serial.
pub const DEFAULT_BAUD_RATE: u32 = 115_200;

/// Maximum packet data length (1-byte length field).
pub const MAX_DATA_LENGTH: usize = 255;
