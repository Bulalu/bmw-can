# Rust Client Workspace

Live client to receive UDP frames from the ESP32 firmware, show a TUI, and later decode via DBC and log to disk.

## Layout
- `client-core/` — types, CSV parsing, (future) DBC decoder and state
- `client-udp/` — async UDP receiver (Tokio), parses into frames
- `client-tui/` — Ratatui app: shows raw frames and basic stats

## Build & Run
- From repo root or `client/` directory:
  - Demo (fake data): `cargo run -p client-tui -- --demo`
- Dual-bus UDP (K‑CAN/PT‑CAN on separate ports):
    - `cargo run -p client-tui -- --host 0.0.0.0 --kcan-port 45454 --ptcan-port 45455 --dbc ../dbc/bmw_e90.dbc`
  - Add OBD (optional third port):
    - `cargo run -p client-tui -- --host 0.0.0.0 --kcan-port 45454 --ptcan-port 45455 --obd-port 45456`
- Quit TUI: press `q`

## Next Steps
- Load and use the DBC to decode key signals
- Add logging (CSV/JSONL) and replay
- Panels for PT-CAN and K-CAN signal groups

## Notes
- Run two firmware devices, each sending to your Mac:
  - K‑CAN: `set can_bps 100000; set host <mac-ip>; set port 45454`
  - PT‑CAN: `set can_bps 500000; set host <mac-ip>; set port 45455`
  - OBD:    `set can_bps 500000; set host <mac-ip>; set port 45456` (OBD firmware build only)
- In the TUI Raw panel, press `b` to cycle bus tab (All/K‑CAN/PT‑CAN), or use the on‑screen tabs.
