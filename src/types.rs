use std::fmt;

/// Provisioning state machine states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImprovState {
    /// Ready to accept Wi-Fi credentials.
    Ready = 0x02,
    /// Attempting Wi-Fi connection with received credentials.
    Provisioning = 0x03,
    /// Successfully connected to Wi-Fi.
    Provisioned = 0x04,
}

impl TryFrom<u8> for ImprovState {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, u8> {
        match value {
            0x02 => Ok(Self::Ready),
            0x03 => Ok(Self::Provisioning),
            0x04 => Ok(Self::Provisioned),
            other => Err(other),
        }
    }
}

impl fmt::Display for ImprovState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready => write!(f, "Ready"),
            Self::Provisioning => write!(f, "Provisioning"),
            Self::Provisioned => write!(f, "Provisioned"),
        }
    }
}

/// Protocol error conditions sent to the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImprovError {
    /// No error.
    None = 0x00,
    /// Malformed RPC packet.
    InvalidRpc = 0x01,
    /// Unrecognized command ID.
    UnknownRpc = 0x02,
    /// Wi-Fi connection failed.
    UnableToConnect = 0x03,
    /// Invalid hostname per RFC 1123.
    BadHostname = 0x05,
    /// Unspecified error.
    Unknown = 0xFF,
}

impl TryFrom<u8> for ImprovError {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, u8> {
        match value {
            0x00 => Ok(Self::None),
            0x01 => Ok(Self::InvalidRpc),
            0x02 => Ok(Self::UnknownRpc),
            0x03 => Ok(Self::UnableToConnect),
            0x05 => Ok(Self::BadHostname),
            0xFF => Ok(Self::Unknown),
            other => Err(other),
        }
    }
}

impl fmt::Display for ImprovError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::InvalidRpc => write!(f, "Invalid RPC"),
            Self::UnknownRpc => write!(f, "Unknown RPC"),
            Self::UnableToConnect => write!(f, "Unable to Connect"),
            Self::BadHostname => write!(f, "Bad Hostname"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Improv serial packet types on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    /// Device → Client: current provisioning state.
    CurrentState = 0x01,
    /// Device → Client: current error state.
    ErrorState = 0x02,
    /// Client → Device: RPC command.
    RpcCommand = 0x03,
    /// Device → Client: RPC result.
    RpcResult = 0x04,
}

impl TryFrom<u8> for PacketType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, u8> {
        match value {
            0x01 => Ok(Self::CurrentState),
            0x02 => Ok(Self::ErrorState),
            0x03 => Ok(Self::RpcCommand),
            0x04 => Ok(Self::RpcResult),
            other => Err(other),
        }
    }
}

impl fmt::Display for PacketType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentState => write!(f, "CurrentState"),
            Self::ErrorState => write!(f, "ErrorState"),
            Self::RpcCommand => write!(f, "RpcCommand"),
            Self::RpcResult => write!(f, "RpcResult"),
        }
    }
}

/// RPC command types from client to device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Send Wi-Fi SSID and password.
    WifiSettings = 0x01,
    /// Request current provisioning state.
    RequestCurrentState = 0x02,
    /// Request device information.
    GetDeviceInfo = 0x03,
    /// Request available Wi-Fi networks.
    GetWifiNetworks = 0x04,
    /// Get or set device hostname.
    GetSetHostname = 0x05,
}

impl TryFrom<u8> for Command {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, u8> {
        match value {
            0x01 => Ok(Self::WifiSettings),
            0x02 => Ok(Self::RequestCurrentState),
            0x03 => Ok(Self::GetDeviceInfo),
            0x04 => Ok(Self::GetWifiNetworks),
            0x05 => Ok(Self::GetSetHostname),
            other => Err(other),
        }
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WifiSettings => write!(f, "WifiSettings"),
            Self::RequestCurrentState => write!(f, "RequestCurrentState"),
            Self::GetDeviceInfo => write!(f, "GetDeviceInfo"),
            Self::GetWifiNetworks => write!(f, "GetWifiNetworks"),
            Self::GetSetHostname => write!(f, "GetSetHostname"),
        }
    }
}

/// Static device metadata configured at initialization.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Firmware name (e.g., "Nightwatch").
    pub firmware_name: String,
    /// Firmware version (e.g., "1.0.0").
    pub firmware_version: String,
    /// Chip family (e.g., "ESP32-S3").
    pub chip_family: String,
    /// Device name (e.g., "Safety Monitor").
    pub device_name: String,
}

/// A scanned Wi-Fi network.
#[derive(Debug, Clone)]
pub struct WifiNetwork {
    /// Network SSID.
    pub ssid: String,
    /// Signal strength in dBm.
    pub rssi: i8,
    /// Whether authentication is required.
    pub auth_required: bool,
}

/// Parsed Wi-Fi credentials from a `WIFI_SETTINGS` command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiCredentials {
    /// Network SSID.
    pub ssid: String,
    /// Network password.
    pub password: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_improv_state_roundtrip() {
        for &(val, expected) in &[
            (0x02u8, ImprovState::Ready),
            (0x03, ImprovState::Provisioning),
            (0x04, ImprovState::Provisioned),
        ] {
            assert_eq!(ImprovState::try_from(val), Ok(expected));
            assert_eq!(expected as u8, val);
        }
        assert!(ImprovState::try_from(0x00).is_err());
        assert!(ImprovState::try_from(0xFF).is_err());
    }

    #[test]
    fn test_improv_error_roundtrip() {
        for &(val, expected) in &[
            (0x00u8, ImprovError::None),
            (0x01, ImprovError::InvalidRpc),
            (0x02, ImprovError::UnknownRpc),
            (0x03, ImprovError::UnableToConnect),
            (0x05, ImprovError::BadHostname),
            (0xFF, ImprovError::Unknown),
        ] {
            assert_eq!(ImprovError::try_from(val), Ok(expected));
            assert_eq!(expected as u8, val);
        }
        assert!(ImprovError::try_from(0x04).is_err());
        assert!(ImprovError::try_from(0x06).is_err());
    }

    #[test]
    fn test_packet_type_roundtrip() {
        for &(val, expected) in &[
            (0x01u8, PacketType::CurrentState),
            (0x02, PacketType::ErrorState),
            (0x03, PacketType::RpcCommand),
            (0x04, PacketType::RpcResult),
        ] {
            assert_eq!(PacketType::try_from(val), Ok(expected));
            assert_eq!(expected as u8, val);
        }
        assert!(PacketType::try_from(0x00).is_err());
        assert!(PacketType::try_from(0x05).is_err());
    }

    #[test]
    fn test_command_roundtrip() {
        for &(val, expected) in &[
            (0x01u8, Command::WifiSettings),
            (0x02, Command::RequestCurrentState),
            (0x03, Command::GetDeviceInfo),
            (0x04, Command::GetWifiNetworks),
            (0x05, Command::GetSetHostname),
        ] {
            assert_eq!(Command::try_from(val), Ok(expected));
            assert_eq!(expected as u8, val);
        }
        assert!(Command::try_from(0x00).is_err());
        assert!(Command::try_from(0xFF).is_err());
    }
}
