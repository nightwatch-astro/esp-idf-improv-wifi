use std::io::{self, Read, Write};

use crate::packet::{
    build_packet, build_rpc_result, parse_rpc_command, parse_string_list, PacketParser, RawPacket,
};
use crate::types::{Command, DeviceInfo, ImprovError, ImprovState, PacketType, WifiNetwork};

type ConnectCb = Box<dyn FnMut(&str, &str) -> Result<String, ()>>;
type ScanCb = Box<dyn FnMut() -> Vec<WifiNetwork>>;
type HostnameCb = Box<dyn FnMut(Option<&str>) -> Result<String, ()>>;

/// Builder for configuring an [`ImprovWifi`] handler.
pub struct ImprovWifiBuilder {
    device_info: DeviceInfo,
    redirect_url: Option<String>,
    on_connect: Option<ConnectCb>,
    on_scan: Option<ScanCb>,
    on_hostname: Option<HostnameCb>,
}

impl ImprovWifiBuilder {
    /// Create a new builder with required device info.
    pub fn new(device_info: DeviceInfo) -> Self {
        Self {
            device_info,
            redirect_url: None,
            on_connect: None,
            on_scan: None,
            on_hostname: None,
        }
    }

    /// Set redirect URL sent after successful provisioning.
    pub fn redirect_url(mut self, url: impl Into<String>) -> Self {
        self.redirect_url = Some(url.into());
        self
    }

    /// Set WiFi connect callback: (ssid, password) -> Result<url, ()>.
    pub fn on_connect(mut self, f: impl FnMut(&str, &str) -> Result<String, ()> + 'static) -> Self {
        self.on_connect = Some(Box::new(f));
        self
    }

    /// Set WiFi scan callback: () -> Vec<WifiNetwork>.
    pub fn on_scan(mut self, f: impl FnMut() -> Vec<WifiNetwork> + 'static) -> Self {
        self.on_scan = Some(Box::new(f));
        self
    }

    /// Set hostname get/set callback: None=get, Some(name)=set.
    pub fn on_hostname(
        mut self,
        f: impl FnMut(Option<&str>) -> Result<String, ()> + 'static,
    ) -> Self {
        self.on_hostname = Some(Box::new(f));
        self
    }

    /// Build the handler with the given transport.
    pub fn build<T: Read + Write>(self, transport: T) -> ImprovWifi<T> {
        ImprovWifi {
            transport,
            state: ImprovState::Ready,
            error: ImprovError::None,
            device_info: self.device_info,
            redirect_url: self.redirect_url,
            on_connect: self.on_connect,
            on_scan: self.on_scan,
            on_hostname: self.on_hostname,
            parser: PacketParser::new(),
            read_buf: [0u8; 256],
        }
    }
}

/// Main ImprovWiFi protocol handler, generic over transport.
pub struct ImprovWifi<T: Read + Write> {
    transport: T,
    state: ImprovState,
    error: ImprovError,
    device_info: DeviceInfo,
    redirect_url: Option<String>,
    on_connect: Option<ConnectCb>,
    on_scan: Option<ScanCb>,
    on_hostname: Option<HostnameCb>,
    parser: PacketParser,
    read_buf: [u8; 256],
}

impl<T: Read + Write> ImprovWifi<T> {
    /// Get current provisioning state.
    pub fn state(&self) -> ImprovState {
        self.state
    }

    /// Get current error state.
    pub fn error(&self) -> ImprovError {
        self.error
    }

    /// Send current state packet (for periodic advertisement).
    pub fn advertise_state(&mut self) -> io::Result<()> {
        self.send_current_state()
    }

    /// Process available data from transport.
    ///
    /// Reads bytes, parses packets, executes commands, writes responses.
    /// Returns `Ok(true)` if a packet was processed, `Ok(false)` if no data.
    pub fn process(&mut self) -> io::Result<bool> {
        let n = match self.transport.read(&mut self.read_buf) {
            Ok(0) => return Ok(false),
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(e) => return Err(e),
        };

        let mut processed = false;
        for i in 0..n {
            if let Some(packet) = self.parser.feed(self.read_buf[i]) {
                self.handle_packet(packet)?;
                processed = true;
            }
        }
        Ok(processed)
    }

    fn handle_packet(&mut self, packet: RawPacket) -> io::Result<()> {
        match packet.packet_type {
            PacketType::RpcCommand => self.handle_rpc_command(&packet.data),
            _ => {
                log::debug!("ignoring non-RPC packet: {}", packet.packet_type);
                Ok(())
            }
        }
    }

    fn handle_rpc_command(&mut self, data: &[u8]) -> io::Result<()> {
        // Clear error state before processing (FR-016)
        self.set_error(ImprovError::None)?;

        let (cmd, payload) = match parse_rpc_command(data) {
            Ok(result) => result,
            Err(e) => {
                log::warn!("invalid RPC packet: {e}");
                return self.set_error(ImprovError::InvalidRpc);
            }
        };

        log::info!("RPC command: {cmd}");

        match cmd {
            Command::WifiSettings => self.handle_wifi_settings(&payload),
            Command::RequestCurrentState => self.handle_request_current_state(),
            Command::GetDeviceInfo => self.handle_get_device_info(),
            Command::GetWifiNetworks => self.handle_get_wifi_networks(),
            Command::GetSetHostname => self.handle_get_set_hostname(&payload),
        }
    }

    fn handle_wifi_settings(&mut self, payload: &[u8]) -> io::Result<()> {
        // Reject if already provisioning
        if self.state == ImprovState::Provisioning {
            return self.set_error(ImprovError::InvalidRpc);
        }

        let strings = match parse_string_list(payload) {
            Ok(s) if s.len() >= 2 => s,
            _ => {
                log::warn!("invalid WIFI_SETTINGS payload");
                return self.set_error(ImprovError::InvalidRpc);
            }
        };

        let ssid = &strings[0];
        let password = &strings[1];

        // Validate SSID non-empty (FR-020)
        if ssid.is_empty() {
            log::warn!("empty SSID in WIFI_SETTINGS");
            return self.set_error(ImprovError::InvalidRpc);
        }

        log::info!("WiFi credentials received for SSID: {ssid}");

        // Transition to Provisioning
        self.set_state(ImprovState::Provisioning)?;

        // Invoke connect callback
        let result = match &mut self.on_connect {
            Some(cb) => cb(ssid, password),
            None => {
                log::warn!("no connect callback configured");
                Err(())
            }
        };

        match result {
            Ok(url) => {
                log::info!("connected successfully, URL: {url}");
                let redirect = self.redirect_url.clone().unwrap_or(url);
                let rpc_data = build_rpc_result(Command::WifiSettings, &[&redirect]);
                self.send_packet(PacketType::RpcResult, &rpc_data)?;
                self.set_state(ImprovState::Provisioned)
            }
            Err(()) => {
                log::warn!("WiFi connection failed");
                self.set_error(ImprovError::UnableToConnect)?;
                self.set_state(ImprovState::Ready)
            }
        }
    }

    fn handle_request_current_state(&mut self) -> io::Result<()> {
        self.send_current_state()
    }

    fn handle_get_device_info(&mut self) -> io::Result<()> {
        let info = &self.device_info;
        let rpc_data = build_rpc_result(
            Command::GetDeviceInfo,
            &[
                &info.firmware_name,
                &info.firmware_version,
                &info.chip_family,
                &info.device_name,
            ],
        );
        self.send_packet(PacketType::RpcResult, &rpc_data)
    }

    fn handle_get_wifi_networks(&mut self) -> io::Result<()> {
        let networks = match &mut self.on_scan {
            Some(cb) => cb(),
            None => {
                log::warn!("no scan callback configured");
                return self.set_error(ImprovError::UnknownRpc);
            }
        };

        for network in &networks {
            let rssi_str = network.rssi.to_string();
            let auth_str = if network.auth_required { "YES" } else { "NO" };
            let rpc_data = build_rpc_result(
                Command::GetWifiNetworks,
                &[&network.ssid, &rssi_str, auth_str],
            );
            self.send_packet(PacketType::RpcResult, &rpc_data)?;
        }

        // Empty terminator
        let rpc_data = build_rpc_result(Command::GetWifiNetworks, &[]);
        self.send_packet(PacketType::RpcResult, &rpc_data)
    }

    fn handle_get_set_hostname(&mut self, payload: &[u8]) -> io::Result<()> {
        let cb = match &mut self.on_hostname {
            Some(cb) => cb,
            None => {
                log::warn!("no hostname callback configured");
                return self.set_error(ImprovError::UnknownRpc);
            }
        };

        if payload.is_empty() {
            // GET hostname
            match cb(None) {
                Ok(hostname) => {
                    let rpc_data = build_rpc_result(Command::GetSetHostname, &[&hostname]);
                    self.send_packet(PacketType::RpcResult, &rpc_data)
                }
                Err(()) => self.set_error(ImprovError::Unknown),
            }
        } else {
            // SET hostname — parse and validate
            let strings = match parse_string_list(payload) {
                Ok(s) if !s.is_empty() => s,
                _ => return self.set_error(ImprovError::InvalidRpc),
            };

            let hostname = &strings[0];
            if !is_valid_hostname(hostname) {
                return self.set_error(ImprovError::BadHostname);
            }

            match cb(Some(hostname)) {
                Ok(result) => {
                    let rpc_data = build_rpc_result(Command::GetSetHostname, &[&result]);
                    self.send_packet(PacketType::RpcResult, &rpc_data)
                }
                Err(()) => self.set_error(ImprovError::Unknown),
            }
        }
    }

    fn set_state(&mut self, state: ImprovState) -> io::Result<()> {
        self.state = state;
        log::info!("state → {state}");
        self.send_current_state()
    }

    fn set_error(&mut self, error: ImprovError) -> io::Result<()> {
        self.error = error;
        if error != ImprovError::None {
            log::warn!("error → {error}");
        }
        self.send_error_state()
    }

    fn send_current_state(&mut self) -> io::Result<()> {
        self.send_packet(PacketType::CurrentState, &[self.state as u8])
    }

    fn send_error_state(&mut self) -> io::Result<()> {
        self.send_packet(PacketType::ErrorState, &[self.error as u8])
    }

    fn send_packet(&mut self, packet_type: PacketType, data: &[u8]) -> io::Result<()> {
        let bytes = build_packet(packet_type, data);
        self.transport.write_all(&bytes)
    }
}

/// Validate a hostname per RFC 1123.
fn is_valid_hostname(hostname: &str) -> bool {
    if hostname.is_empty() || hostname.len() > 253 {
        return false;
    }
    for label in hostname.split('.') {
        if label.is_empty() || label.len() > 63 {
            return false;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return false;
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::build_packet;
    use std::io::Cursor;

    fn make_device_info() -> DeviceInfo {
        DeviceInfo {
            firmware_name: "Nightwatch".into(),
            firmware_version: "1.0.0".into(),
            chip_family: "ESP32-S3".into(),
            device_name: "Safety Monitor".into(),
        }
    }

    /// Build a WIFI_SETTINGS RPC command packet.
    fn make_wifi_settings_packet(ssid: &str, password: &str) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.push(Command::WifiSettings as u8);
        let mut string_data = Vec::new();
        string_data.push(ssid.len() as u8);
        string_data.extend_from_slice(ssid.as_bytes());
        string_data.push(password.len() as u8);
        string_data.extend_from_slice(password.as_bytes());
        payload.push(string_data.len() as u8);
        payload.extend_from_slice(&string_data);
        build_packet(PacketType::RpcCommand, &payload)
    }

    /// Build a simple RPC command packet (no payload).
    fn make_simple_command_packet(cmd: Command) -> Vec<u8> {
        let payload = vec![cmd as u8, 0x00]; // command + 0 data length
        build_packet(PacketType::RpcCommand, &payload)
    }

    /// Build an RPC command with custom payload.
    fn make_command_with_payload(cmd: Command, data: &[u8]) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.push(cmd as u8);
        payload.push(data.len() as u8);
        payload.extend_from_slice(data);
        build_packet(PacketType::RpcCommand, &payload)
    }

    /// Helper: create a handler, write input bytes, process, return output bytes.
    fn run_handler(
        input: &[u8],
        on_connect: Option<Box<dyn FnMut(&str, &str) -> Result<String, ()>>>,
        on_scan: Option<Box<dyn FnMut() -> Vec<WifiNetwork>>>,
    ) -> (ImprovWifi<Cursor<Vec<u8>>>, Vec<u8>) {
        let mut transport = Cursor::new(Vec::new());
        transport.get_mut().extend_from_slice(input);
        transport.set_position(0);

        let mut builder = ImprovWifiBuilder::new(make_device_info());
        if let Some(cb) = on_connect {
            builder.on_connect = Some(cb);
        }
        if let Some(cb) = on_scan {
            builder.on_scan = Some(cb);
        }
        let mut handler = builder.build(transport);
        let _ = handler.process();

        let output = handler.transport.get_ref().clone();
        // Output starts after the input bytes
        let output_start = input.len();
        let written = if output.len() > output_start {
            output[output_start..].to_vec()
        } else {
            Vec::new()
        };
        (handler, written)
    }

    /// Parse all packets from output bytes.
    fn parse_output_packets(data: &[u8]) -> Vec<RawPacket> {
        let mut parser = PacketParser::new();
        let mut packets = Vec::new();
        for &byte in data {
            if let Some(pkt) = parser.feed(byte) {
                packets.push(pkt);
            }
        }
        packets
    }

    #[test]
    fn test_provisioning_success() {
        let input = make_wifi_settings_packet("MyNetwork", "secret123");
        let connect_ssid = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let connect_password = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let ssid_clone = connect_ssid.clone();
        let pass_clone = connect_password.clone();

        let (handler, output) = run_handler(
            &input,
            Some(Box::new(move |ssid, password| {
                *ssid_clone.lock().unwrap() = ssid.to_string();
                *pass_clone.lock().unwrap() = password.to_string();
                Ok("http://192.168.1.100".into())
            })),
            None,
        );

        assert_eq!(*connect_ssid.lock().unwrap(), "MyNetwork");
        assert_eq!(*connect_password.lock().unwrap(), "secret123");
        assert_eq!(handler.state(), ImprovState::Provisioned);
        assert_eq!(handler.error(), ImprovError::None);

        let packets = parse_output_packets(&output);
        // Expected: ErrorState(None), CurrentState(Provisioning), RpcResult, CurrentState(Provisioned)
        assert!(packets.len() >= 3);

        // Find the CurrentState packets
        let state_packets: Vec<_> = packets
            .iter()
            .filter(|p| p.packet_type == PacketType::CurrentState)
            .collect();
        assert!(state_packets.len() >= 2);
        assert_eq!(state_packets[0].data, vec![ImprovState::Provisioning as u8]);
        assert_eq!(
            state_packets[state_packets.len() - 1].data,
            vec![ImprovState::Provisioned as u8]
        );

        // Find the RPC result
        let rpc_packets: Vec<_> = packets
            .iter()
            .filter(|p| p.packet_type == PacketType::RpcResult)
            .collect();
        assert_eq!(rpc_packets.len(), 1);
    }

    #[test]
    fn test_provisioning_failure() {
        let input = make_wifi_settings_packet("BadNetwork", "wrong");
        let (handler, output) =
            run_handler(&input, Some(Box::new(|_ssid, _password| Err(()))), None);

        assert_eq!(handler.state(), ImprovState::Ready);
        assert_eq!(handler.error(), ImprovError::UnableToConnect);

        let packets = parse_output_packets(&output);
        // Should contain ErrorState(UnableToConnect)
        let error_packets: Vec<_> = packets
            .iter()
            .filter(|p| {
                p.packet_type == PacketType::ErrorState
                    && !p.data.is_empty()
                    && p.data[0] == ImprovError::UnableToConnect as u8
            })
            .collect();
        assert!(!error_packets.is_empty());

        // Should end in Ready state
        let state_packets: Vec<_> = packets
            .iter()
            .filter(|p| p.packet_type == PacketType::CurrentState)
            .collect();
        assert!(!state_packets.is_empty());
        assert_eq!(
            state_packets.last().unwrap().data,
            vec![ImprovState::Ready as u8]
        );
    }

    #[test]
    fn test_empty_ssid_rejected() {
        let input = make_wifi_settings_packet("", "password");
        let connect_called = std::sync::Arc::new(std::sync::Mutex::new(false));
        let called_clone = connect_called.clone();

        let (handler, output) = run_handler(
            &input,
            Some(Box::new(move |_ssid, _password| {
                *called_clone.lock().unwrap() = true;
                Ok("url".into())
            })),
            None,
        );

        assert!(
            !*connect_called.lock().unwrap(),
            "connect should not be called"
        );
        assert_eq!(handler.error(), ImprovError::InvalidRpc);

        let packets = parse_output_packets(&output);
        let error_packets: Vec<_> = packets
            .iter()
            .filter(|p| {
                p.packet_type == PacketType::ErrorState
                    && !p.data.is_empty()
                    && p.data[0] == ImprovError::InvalidRpc as u8
            })
            .collect();
        assert!(!error_packets.is_empty());
    }

    #[test]
    fn test_advertise_state() {
        let transport = Cursor::new(Vec::new());
        let mut handler = ImprovWifiBuilder::new(make_device_info()).build(transport);
        handler.advertise_state().unwrap();

        let output = handler.transport.get_ref().clone();
        let packets = parse_output_packets(&output);
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].packet_type, PacketType::CurrentState);
        assert_eq!(packets[0].data, vec![ImprovState::Ready as u8]);
    }

    #[test]
    fn test_request_current_state() {
        let input = make_simple_command_packet(Command::RequestCurrentState);
        let (_handler, output) = run_handler(&input, None, None);

        let packets = parse_output_packets(&output);
        let state_packets: Vec<_> = packets
            .iter()
            .filter(|p| p.packet_type == PacketType::CurrentState)
            .collect();
        assert!(!state_packets.is_empty());
        assert_eq!(state_packets[0].data, vec![ImprovState::Ready as u8]);
    }

    #[test]
    fn test_get_device_info() {
        let input = make_simple_command_packet(Command::GetDeviceInfo);
        let (_handler, output) = run_handler(&input, None, None);

        let packets = parse_output_packets(&output);
        let rpc_packets: Vec<_> = packets
            .iter()
            .filter(|p| p.packet_type == PacketType::RpcResult)
            .collect();
        assert_eq!(rpc_packets.len(), 1);

        let (cmd, data) = parse_rpc_command(&rpc_packets[0].data).unwrap();
        assert_eq!(cmd, Command::GetDeviceInfo);
        let strings = parse_string_list(&data).unwrap();
        assert_eq!(
            strings,
            vec!["Nightwatch", "1.0.0", "ESP32-S3", "Safety Monitor"]
        );
    }

    #[test]
    fn test_get_wifi_networks() {
        let input = make_simple_command_packet(Command::GetWifiNetworks);
        let (_handler, output) = run_handler(
            &input,
            None,
            Some(Box::new(|| {
                vec![
                    WifiNetwork {
                        ssid: "Home".into(),
                        rssi: -45,
                        auth_required: true,
                    },
                    WifiNetwork {
                        ssid: "Guest".into(),
                        rssi: -70,
                        auth_required: false,
                    },
                ]
            })),
        );

        let packets = parse_output_packets(&output);
        let rpc_packets: Vec<_> = packets
            .iter()
            .filter(|p| p.packet_type == PacketType::RpcResult)
            .collect();
        // 2 networks + 1 empty terminator
        assert_eq!(rpc_packets.len(), 3);

        // First network
        let (cmd, data) = parse_rpc_command(&rpc_packets[0].data).unwrap();
        assert_eq!(cmd, Command::GetWifiNetworks);
        let strings = parse_string_list(&data).unwrap();
        assert_eq!(strings, vec!["Home", "-45", "YES"]);

        // Second network
        let (_, data) = parse_rpc_command(&rpc_packets[1].data).unwrap();
        let strings = parse_string_list(&data).unwrap();
        assert_eq!(strings, vec!["Guest", "-70", "NO"]);

        // Empty terminator
        let (_, data) = parse_rpc_command(&rpc_packets[2].data).unwrap();
        let strings = parse_string_list(&data).unwrap();
        assert!(strings.is_empty());
    }

    #[test]
    fn test_get_wifi_networks_empty() {
        let input = make_simple_command_packet(Command::GetWifiNetworks);
        let (_handler, output) = run_handler(&input, None, Some(Box::new(Vec::new)));

        let packets = parse_output_packets(&output);
        let rpc_packets: Vec<_> = packets
            .iter()
            .filter(|p| p.packet_type == PacketType::RpcResult)
            .collect();
        assert_eq!(rpc_packets.len(), 1); // Just the empty terminator
    }

    #[test]
    fn test_get_wifi_networks_no_callback() {
        let input = make_simple_command_packet(Command::GetWifiNetworks);
        let (handler, output) = run_handler(&input, None, None);

        assert_eq!(handler.error(), ImprovError::UnknownRpc);
        let packets = parse_output_packets(&output);
        let error_packets: Vec<_> = packets
            .iter()
            .filter(|p| {
                p.packet_type == PacketType::ErrorState
                    && !p.data.is_empty()
                    && p.data[0] == ImprovError::UnknownRpc as u8
            })
            .collect();
        assert!(!error_packets.is_empty());
    }

    #[test]
    fn test_unknown_command() {
        // Build a packet with unknown command ID 0xFF
        let payload = vec![0xFF, 0x00];
        let input = build_packet(PacketType::RpcCommand, &payload);
        let transport = Cursor::new(Vec::new());
        let mut handler = ImprovWifiBuilder::new(make_device_info()).build(transport);
        handler.transport.get_mut().extend_from_slice(&input);
        handler.transport.set_position(0);
        let _ = handler.process();

        assert_eq!(handler.error(), ImprovError::InvalidRpc);
    }

    #[test]
    fn test_error_cleared_on_new_command() {
        // First trigger an error
        let input1 = make_wifi_settings_packet("", "pass");
        let transport = Cursor::new(Vec::new());
        let mut handler = ImprovWifiBuilder::new(make_device_info())
            .on_connect(|_, _| Ok("url".into()))
            .build(transport);

        handler.transport.get_mut().extend_from_slice(&input1);
        handler.transport.set_position(0);
        let _ = handler.process();
        assert_eq!(handler.error(), ImprovError::InvalidRpc);

        // Now send a valid command — error should clear before processing
        let input2 = make_simple_command_packet(Command::RequestCurrentState);
        let pos = handler.transport.get_ref().len();
        handler.transport.get_mut().extend_from_slice(&input2);
        handler.transport.set_position(pos as u64);
        let _ = handler.process();

        // Error was cleared (set to None before processing the new command)
        assert_eq!(handler.error(), ImprovError::None);
    }

    #[test]
    fn test_full_provisioning_flow() {
        let transport = Cursor::new(Vec::new());
        let mut handler = ImprovWifiBuilder::new(make_device_info())
            .redirect_url("http://nightwatch.local")
            .on_connect(|_ssid, _password| Ok("http://192.168.1.100".into()))
            .on_scan(|| {
                vec![WifiNetwork {
                    ssid: "TestNet".into(),
                    rssi: -50,
                    auth_required: true,
                }]
            })
            .build(transport);

        // 1. Advertise state
        handler.advertise_state().unwrap();
        assert_eq!(handler.state(), ImprovState::Ready);

        // 2. GET_DEVICE_INFO
        let pkt = make_simple_command_packet(Command::GetDeviceInfo);
        let pos = handler.transport.get_ref().len();
        handler.transport.get_mut().extend_from_slice(&pkt);
        handler.transport.set_position(pos as u64);
        assert!(handler.process().unwrap());

        // 3. GET_WIFI_NETWORKS
        let pkt = make_simple_command_packet(Command::GetWifiNetworks);
        let pos = handler.transport.get_ref().len();
        handler.transport.get_mut().extend_from_slice(&pkt);
        handler.transport.set_position(pos as u64);
        assert!(handler.process().unwrap());

        // 4. WIFI_SETTINGS
        let pkt = make_wifi_settings_packet("TestNet", "password123");
        let pos = handler.transport.get_ref().len();
        handler.transport.get_mut().extend_from_slice(&pkt);
        handler.transport.set_position(pos as u64);
        assert!(handler.process().unwrap());

        assert_eq!(handler.state(), ImprovState::Provisioned);
        assert_eq!(handler.error(), ImprovError::None);
    }

    // Hostname tests
    #[test]
    fn test_get_hostname() {
        let input = make_simple_command_packet(Command::GetSetHostname);
        let transport = Cursor::new(Vec::new());
        let mut handler = ImprovWifiBuilder::new(make_device_info())
            .on_hostname(|arg| match arg {
                None => Ok("nightwatch-01".into()),
                Some(name) => Ok(name.to_string()),
            })
            .build(transport);

        handler.transport.get_mut().extend_from_slice(&input);
        handler.transport.set_position(0);
        let _ = handler.process();

        let output = handler.transport.get_ref().clone();
        let output_start = input.len();
        let written = &output[output_start..];
        let packets = parse_output_packets(written);
        let rpc_packets: Vec<_> = packets
            .iter()
            .filter(|p| p.packet_type == PacketType::RpcResult)
            .collect();
        assert_eq!(rpc_packets.len(), 1);
        let (cmd, data) = parse_rpc_command(&rpc_packets[0].data).unwrap();
        assert_eq!(cmd, Command::GetSetHostname);
        let strings = parse_string_list(&data).unwrap();
        assert_eq!(strings, vec!["nightwatch-01"]);
    }

    #[test]
    fn test_set_hostname_valid() {
        // Build SET_HOSTNAME with "my-device"
        let mut string_data = Vec::new();
        string_data.push(9u8); // "my-device".len()
        string_data.extend_from_slice(b"my-device");
        let input = make_command_with_payload(Command::GetSetHostname, &string_data);

        let transport = Cursor::new(Vec::new());
        let mut handler = ImprovWifiBuilder::new(make_device_info())
            .on_hostname(|arg| match arg {
                None => Ok("old".into()),
                Some(name) => Ok(name.to_string()),
            })
            .build(transport);

        handler.transport.get_mut().extend_from_slice(&input);
        handler.transport.set_position(0);
        let _ = handler.process();

        let output = handler.transport.get_ref().clone();
        let written = &output[input.len()..];
        let packets = parse_output_packets(written);
        let rpc_packets: Vec<_> = packets
            .iter()
            .filter(|p| p.packet_type == PacketType::RpcResult)
            .collect();
        assert_eq!(rpc_packets.len(), 1);
        let (_, data) = parse_rpc_command(&rpc_packets[0].data).unwrap();
        let strings = parse_string_list(&data).unwrap();
        assert_eq!(strings, vec!["my-device"]);
    }

    #[test]
    fn test_set_hostname_invalid() {
        let mut string_data = Vec::new();
        string_data.push(17u8);
        string_data.extend_from_slice(b"invalid hostname!");
        let input = make_command_with_payload(Command::GetSetHostname, &string_data);

        let transport = Cursor::new(Vec::new());
        let mut handler = ImprovWifiBuilder::new(make_device_info())
            .on_hostname(|_| Ok("should-not-be-called".into()))
            .build(transport);

        handler.transport.get_mut().extend_from_slice(&input);
        handler.transport.set_position(0);
        let _ = handler.process();

        assert_eq!(handler.error(), ImprovError::BadHostname);
    }

    #[test]
    fn test_hostname_rfc1123_validation() {
        // Valid hostnames
        assert!(is_valid_hostname("nightwatch"));
        assert!(is_valid_hostname("my-device"));
        assert!(is_valid_hostname("a.b.c"));
        assert!(is_valid_hostname("host-01.local"));

        // Invalid hostnames
        assert!(!is_valid_hostname(""));
        assert!(!is_valid_hostname("-leading"));
        assert!(!is_valid_hostname("trailing-"));
        assert!(!is_valid_hostname("invalid hostname!"));
        assert!(!is_valid_hostname("has space"));
        assert!(!is_valid_hostname(&"a".repeat(254)));

        // Label > 63 chars
        let long_label = "a".repeat(64);
        assert!(!is_valid_hostname(&long_label));

        // Label exactly 63 chars
        let ok_label = "a".repeat(63);
        assert!(is_valid_hostname(&ok_label));
    }
}
