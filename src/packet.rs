use std::fmt;

use crate::types::{Command, PacketType};
use crate::{HEADER, MAX_DATA_LENGTH, PROTOCOL_VERSION};

/// Error returned when parsing a packet or RPC payload fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub &'static str);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error: {}", self.0)
    }
}

impl std::error::Error for ParseError {}

/// A parsed Improv serial packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPacket {
    pub packet_type: PacketType,
    pub data: Vec<u8>,
}

/// Incremental packet parser that accumulates bytes and emits complete packets.
///
/// Handles header synchronization, checksum validation, and partial packet
/// reassembly. Feed bytes one at a time via [`feed`] or in bulk via [`feed_all`].
pub struct PacketParser {
    buf: Vec<u8>,
    state: ParseState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    /// Scanning for header byte at position 0..6.
    Header(usize),
    /// Reading version byte.
    Version,
    /// Reading type byte.
    Type,
    /// Reading length byte.
    Length,
    /// Reading data bytes (remaining count).
    Data(usize),
    /// Reading checksum byte.
    Checksum,
}

impl PacketParser {
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(HEADER.len() + 3 + MAX_DATA_LENGTH + 1),
            state: ParseState::Header(0),
        }
    }

    /// Reset parser state, discarding any partial packet.
    pub fn reset(&mut self) {
        self.buf.clear();
        self.state = ParseState::Header(0);
    }

    /// Feed a single byte into the parser.
    ///
    /// Returns `Some(RawPacket)` when a complete, valid packet has been parsed.
    /// Returns `None` if more bytes are needed or the byte was discarded.
    pub fn feed(&mut self, byte: u8) -> Option<RawPacket> {
        match self.state {
            ParseState::Header(pos) => {
                if byte == HEADER[pos] {
                    self.buf.push(byte);
                    if pos + 1 == HEADER.len() {
                        self.state = ParseState::Version;
                    } else {
                        self.state = ParseState::Header(pos + 1);
                    }
                } else {
                    // Resync: check if this byte starts a new header
                    self.buf.clear();
                    if byte == HEADER[0] {
                        self.buf.push(byte);
                        self.state = ParseState::Header(1);
                    } else {
                        self.state = ParseState::Header(0);
                    }
                }
                None
            }
            ParseState::Version => {
                self.buf.push(byte);
                if byte == PROTOCOL_VERSION {
                    self.state = ParseState::Type;
                } else {
                    log::warn!("unsupported protocol version: {byte}");
                    self.reset();
                }
                None
            }
            ParseState::Type => {
                self.buf.push(byte);
                match PacketType::try_from(byte) {
                    Ok(_) => self.state = ParseState::Length,
                    Err(_) => {
                        log::warn!("unknown packet type: 0x{byte:02X}");
                        self.reset();
                    }
                }
                None
            }
            ParseState::Length => {
                self.buf.push(byte);
                let len = byte as usize;
                if len == 0 {
                    self.state = ParseState::Checksum;
                } else {
                    self.state = ParseState::Data(len);
                }
                None
            }
            ParseState::Data(remaining) => {
                self.buf.push(byte);
                if remaining <= 1 {
                    self.state = ParseState::Checksum;
                } else {
                    self.state = ParseState::Data(remaining - 1);
                }
                None
            }
            ParseState::Checksum => {
                let expected: u8 = self.buf.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
                if byte == expected {
                    let packet_type =
                        PacketType::try_from(self.buf[HEADER.len() + 1]).expect("validated");
                    let data_start = HEADER.len() + 3; // header + version + type + length
                    let data = self.buf[data_start..].to_vec();
                    self.reset();
                    Some(RawPacket { packet_type, data })
                } else {
                    log::warn!("checksum mismatch: expected 0x{expected:02X}, got 0x{byte:02X}");
                    self.reset();
                    None
                }
            }
        }
    }

    /// Feed multiple bytes, returning the first complete packet found (if any).
    pub fn feed_all(&mut self, bytes: &[u8]) -> Option<RawPacket> {
        for &byte in bytes {
            if let Some(packet) = self.feed(byte) {
                return Some(packet);
            }
        }
        None
    }
}

impl Default for PacketParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a complete wire-format Improv serial packet.
pub fn build_packet(packet_type: PacketType, data: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(HEADER.len() + 3 + data.len() + 1);
    packet.extend_from_slice(&HEADER);
    packet.push(PROTOCOL_VERSION);
    packet.push(packet_type as u8);
    packet.push(data.len() as u8);
    packet.extend_from_slice(data);
    let checksum: u8 = packet.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    packet.push(checksum);
    packet
}

/// Build an RPC Result payload with length-prefixed UTF-8 strings.
///
/// Format: command_id, total_data_length, [len, string_bytes...]...
pub fn build_rpc_result(command: Command, strings: &[&str]) -> Vec<u8> {
    let mut payload = Vec::new();
    // String data (length-prefixed)
    let mut string_data = Vec::new();
    for s in strings {
        string_data.push(s.len() as u8);
        string_data.extend_from_slice(s.as_bytes());
    }
    payload.push(command as u8);
    payload.push(string_data.len() as u8);
    payload.extend_from_slice(&string_data);
    payload
}

/// Parse an RPC command from packet data.
///
/// Returns the command type and the remaining payload bytes.
pub fn parse_rpc_command(data: &[u8]) -> Result<(Command, Vec<u8>), ParseError> {
    if data.is_empty() {
        return Err(ParseError("empty RPC data"));
    }
    let cmd = Command::try_from(data[0]).map_err(|_| ParseError("unknown command"))?;
    if data.len() < 2 {
        return Ok((cmd, Vec::new()));
    }
    let payload_len = data[1] as usize;
    if data.len() < 2 + payload_len {
        return Err(ParseError("truncated RPC payload"));
    }
    Ok((cmd, data[2..2 + payload_len].to_vec()))
}

/// Parse length-prefixed strings from an RPC payload (e.g., WIFI_SETTINGS).
pub fn parse_string_list(data: &[u8]) -> Result<Vec<String>, ParseError> {
    let mut strings = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let len = data[pos] as usize;
        pos += 1;
        if pos + len > data.len() {
            return Err(ParseError("truncated string"));
        }
        let s = String::from_utf8(data[pos..pos + len].to_vec())
            .map_err(|_| ParseError("invalid UTF-8"))?;
        strings.push(s);
        pos += len;
    }
    Ok(strings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_rpc_wifi_settings(ssid: &str, password: &str) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.push(Command::WifiSettings as u8);
        let data_len = 1 + ssid.len() + 1 + password.len();
        payload.push(data_len as u8);
        payload.push(ssid.len() as u8);
        payload.extend_from_slice(ssid.as_bytes());
        payload.push(password.len() as u8);
        payload.extend_from_slice(password.as_bytes());
        build_packet(PacketType::RpcCommand, &payload)
    }

    #[test]
    fn test_build_and_parse_current_state() {
        let data = [0x02]; // Ready
        let packet_bytes = build_packet(PacketType::CurrentState, &data);
        let mut parser = PacketParser::new();
        let result = parser.feed_all(&packet_bytes);
        assert_eq!(
            result,
            Some(RawPacket {
                packet_type: PacketType::CurrentState,
                data: vec![0x02],
            })
        );
    }

    #[test]
    fn test_build_and_parse_error_state() {
        let data = [0x03]; // UnableToConnect
        let packet_bytes = build_packet(PacketType::ErrorState, &data);
        let mut parser = PacketParser::new();
        let result = parser.feed_all(&packet_bytes);
        assert_eq!(
            result,
            Some(RawPacket {
                packet_type: PacketType::ErrorState,
                data: vec![0x03],
            })
        );
    }

    #[test]
    fn test_build_and_parse_rpc_command() {
        let packet_bytes = make_rpc_wifi_settings("TestSSID", "pass123");
        let mut parser = PacketParser::new();
        let result = parser.feed_all(&packet_bytes).unwrap();
        assert_eq!(result.packet_type, PacketType::RpcCommand);

        let (cmd, payload) = parse_rpc_command(&result.data).unwrap();
        assert_eq!(cmd, Command::WifiSettings);

        let strings = parse_string_list(&payload).unwrap();
        assert_eq!(strings, vec!["TestSSID", "pass123"]);
    }

    #[test]
    fn test_build_and_parse_rpc_result() {
        let payload = build_rpc_result(Command::GetDeviceInfo, &["FW", "1.0", "ESP32", "Dev"]);
        let packet_bytes = build_packet(PacketType::RpcResult, &payload);
        let mut parser = PacketParser::new();
        let result = parser.feed_all(&packet_bytes).unwrap();
        assert_eq!(result.packet_type, PacketType::RpcResult);

        let (cmd, data) = parse_rpc_command(&result.data).unwrap();
        assert_eq!(cmd, Command::GetDeviceInfo);

        let strings = parse_string_list(&data).unwrap();
        assert_eq!(strings, vec!["FW", "1.0", "ESP32", "Dev"]);
    }

    #[test]
    fn test_checksum_correct() {
        let packet_bytes = build_packet(PacketType::CurrentState, &[0x04]);
        // Verify checksum is last byte and is sum of all preceding bytes
        let checksum = packet_bytes[packet_bytes.len() - 1];
        let expected: u8 = packet_bytes[..packet_bytes.len() - 1]
            .iter()
            .fold(0u8, |acc, &b| acc.wrapping_add(b));
        assert_eq!(checksum, expected);
    }

    #[test]
    fn test_bad_checksum_rejected() {
        let mut packet_bytes = build_packet(PacketType::CurrentState, &[0x02]);
        // Corrupt checksum
        let last = packet_bytes.len() - 1;
        packet_bytes[last] = packet_bytes[last].wrapping_add(1);

        let mut parser = PacketParser::new();
        let result = parser.feed_all(&packet_bytes);
        assert_eq!(result, None);
    }

    #[test]
    fn test_header_sync_with_garbage() {
        let mut parser = PacketParser::new();
        // Feed 50 garbage bytes
        for i in 0..50u8 {
            assert_eq!(parser.feed(i), None);
        }
        // Then a valid packet
        let packet_bytes = build_packet(PacketType::CurrentState, &[0x02]);
        let result = parser.feed_all(&packet_bytes);
        assert!(result.is_some());
        assert_eq!(result.unwrap().data, vec![0x02]);
    }

    #[test]
    fn test_incremental_byte_by_byte() {
        let packet_bytes = build_packet(PacketType::CurrentState, &[0x04]);
        let mut parser = PacketParser::new();
        let mut result = None;
        for &byte in &packet_bytes {
            if let Some(pkt) = parser.feed(byte) {
                result = Some(pkt);
            }
        }
        assert_eq!(
            result,
            Some(RawPacket {
                packet_type: PacketType::CurrentState,
                data: vec![0x04],
            })
        );
    }

    #[test]
    fn test_empty_data_packet() {
        // Length = 0
        let packet_bytes = build_packet(PacketType::ErrorState, &[]);
        let mut parser = PacketParser::new();
        let result = parser.feed_all(&packet_bytes);
        assert_eq!(
            result,
            Some(RawPacket {
                packet_type: PacketType::ErrorState,
                data: vec![],
            })
        );
    }

    #[test]
    fn test_rpc_result_empty_strings() {
        let payload = build_rpc_result(Command::GetWifiNetworks, &[]);
        let (cmd, data) = parse_rpc_command(&payload).unwrap();
        assert_eq!(cmd, Command::GetWifiNetworks);
        let strings = parse_string_list(&data).unwrap();
        assert!(strings.is_empty());
    }

    #[test]
    fn test_rpc_result_single_string() {
        let payload = build_rpc_result(Command::WifiSettings, &["http://192.168.1.1"]);
        let (cmd, data) = parse_rpc_command(&payload).unwrap();
        assert_eq!(cmd, Command::WifiSettings);
        let strings = parse_string_list(&data).unwrap();
        assert_eq!(strings, vec!["http://192.168.1.1"]);
    }

    #[test]
    fn test_parse_wifi_settings_payload() {
        let mut data = Vec::new();
        data.push(8); // SSID length
        data.extend_from_slice(b"HomeWiFi");
        data.push(6); // password length
        data.extend_from_slice(b"secret");

        let strings = parse_string_list(&data).unwrap();
        assert_eq!(strings, vec!["HomeWiFi", "secret"]);
    }

    #[test]
    fn test_parse_string_list_truncated() {
        let data = vec![5, b'H', b'e']; // Claims length 5 but only 2 bytes
        assert!(parse_string_list(&data).is_err());
    }

    #[test]
    fn test_two_packets_in_sequence() {
        let pkt1 = build_packet(PacketType::CurrentState, &[0x02]);
        let pkt2 = build_packet(PacketType::CurrentState, &[0x04]);
        let mut combined = pkt1.clone();
        combined.extend_from_slice(&pkt2);

        let mut parser = PacketParser::new();
        let mut results = Vec::new();
        for &byte in &combined {
            if let Some(pkt) = parser.feed(byte) {
                results.push(pkt);
            }
        }
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].data, vec![0x02]);
        assert_eq!(results[1].data, vec![0x04]);
    }
}
