# Firmware (ESP32) Scaffold

This directory contains a PlatformIO project for the ESP32 CAN sniffer/forwarder. It is a scaffold only; logic is stubbed for now.

## Requirements
- PlatformIO Core (VS Code extension or `pipx install platformio`)
- ESP32 dev board (Arduino framework)

## Build & Upload
- Select environment:
  - PT-CAN (500 kbps): `pio run -e esp32dev-ptcan`
  - K-CAN (100 kbps): `pio run -e esp32dev-kcan`
- Upload (example PT-CAN): `pio run -t upload -e esp32dev-ptcan`
- Serial monitor: `pio device monitor -b 115200`
  
Host listener options:
- Quick check with netcat: `nc -u -l 45454`
- Python helper (parsed output): `python3 tools/udp_recv.py --port 45454`

## Configuration
Defaults via build flags in `platformio.ini`:
- `CAN_BPS` (500000 or 100000)
- `CAN_TX` (default 13)
- `CAN_RX` (default 5)
- `UDP_HOST` (default "192.168.1.5")
- `UDP_PORT` (default 45454)

Optional Wi‑Fi credentials for auto-connect (add to env build_flags):
- `WIFI_SSID` and `WIFI_PASS`

Example:
```
build_flags =
  -DCAN_BPS=500000
  -DCAN_TX=13
  -DCAN_RX=5
  -DUDP_PORT=45454
  -DUDP_HOST=\"192.168.1.5\"
  -DWIFI_SSID=\"YourSSID\"
  -DWIFI_PASS=\"YourPass\"
```

Runtime CLI over Serial (early stub):
- `help` — list commands
- `get` — print current config
- `set <key> <value>` — keys: `can_bps`, `host`, `port`, `wifi_ssid`, `wifi_pass`
- `save` — persist to NVS (survives reboot)
- `net reconnect` — reapply Wi‑Fi/UDP without reboot
- `reboot` — restart device
- `selftest on|off` — enable/disable synthetic frames
- `selftest once <n>` — emit N frames immediately for testing

## Layout
- `src/main.cpp` — boot & wiring
- `include/*.hpp` — modules: CAN, UDP, CLI, config
- `src/*.cpp` — stub implementations
- `platformio.ini` — build environments

> Implemented: TWAI init/RX, UDP send, CLI set/save/reboot, NVS persistence, runtime UDP reconnect, and self-test generator.

## Notes
- The CAN controller runs in listen-only mode (no ACK, no transmit). Safe for tapping an active vehicle bus.
