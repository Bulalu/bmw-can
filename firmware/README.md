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

## Configuration
Defaults via build flags in `platformio.ini`:
- `CAN_BPS` (500000 or 100000)
- `CAN_TX` (default 13)
- `CAN_RX` (default 5)
- `UDP_HOST` (default "192.168.1.100")
- `UDP_PORT` (default 45454)

Runtime CLI over Serial (early stub):
- `help` — list commands
- `get` — print current config

## Layout
- `src/main.cpp` — boot & wiring
- `include/*.hpp` — modules: CAN, UDP, CLI, config
- `src/*.cpp` — stub implementations
- `platformio.ini` — build environments

> Next steps: implement TWAI (CAN) init/read, Wi‑Fi + UDP send, and a simple CLI to set host/port and CAN bitrate, then persist via NVS.
