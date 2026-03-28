# esp-idf-improv-wifi

Rust implementation of the [Improv WiFi](https://www.improv-wifi.com/) serial provisioning protocol.

Transport-agnostic core that works on any `std::io::Read + Write` stream, with an optional ESP-IDF UART adapter for ESP32.

## Features

- **Full Improv v1 protocol**: packet framing, state machine, all 5 RPC commands
- **Transport-agnostic**: core logic works with any `Read + Write` stream
- **Host-testable**: 34 tests run on desktop without ESP32 hardware
- **ESP-IDF integration**: optional UART transport via `esp-idf-svc` feature flag
- **Builder API**: fluent configuration with callbacks for WiFi, scanning, and hostname

## Usage

```bash
cargo add esp-idf-improv-wifi

# For ESP32 UART transport:
cargo add esp-idf-improv-wifi --features esp-idf-svc
```

```rust
use esp_idf_improv_wifi::{ImprovWifiBuilder, DeviceInfo};
use std::io::Cursor;

let info = DeviceInfo {
    firmware_name: "MyFirmware".into(),
    firmware_version: "1.0.0".into(),
    chip_family: "ESP32-S3".into(),
    device_name: "My Device".into(),
};

let serial = Cursor::new(Vec::new()); // replace with real transport

let mut handler = ImprovWifiBuilder::new(info)
    .redirect_url("http://192.168.1.100")
    .on_connect(|ssid, password| {
        println!("Connecting to {ssid}...");
        Ok("http://192.168.1.100".into())
    })
    .on_scan(|| vec![]) // optional: return scanned networks
    .build(serial);

// Main loop
loop {
    handler.advertise_state().unwrap();
    match handler.process() {
        Ok(true) => {} // packet processed
        Ok(false) => {} // no data available
        Err(e) => eprintln!("Error: {e}"),
    }
    # break; // remove in real code
}
```

## RPC Commands

| Command | ID | Description |
|---------|-----|-------------|
| WIFI_SETTINGS | 0x01 | Send WiFi SSID and password |
| REQUEST_CURRENT_STATE | 0x02 | Request current provisioning state |
| GET_DEVICE_INFO | 0x03 | Request device metadata |
| GET_WIFI_NETWORKS | 0x04 | Request WiFi scan results |
| GET_SET_HOSTNAME | 0x05 | Get or set device hostname |

## Feature Flags

| Feature | Description |
|---------|-------------|
| `esp-idf-svc` | Enables `UartTransport` for ESP-IDF VFS UART |

## Testing

```bash
cargo test                    # all protocol tests on host
cargo clippy -- -D warnings   # lint
cargo fmt --check             # format check
```

## Protocol Reference

See the [Improv WiFi serial specification](https://www.improv-wifi.com/serial/) for full protocol details.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
