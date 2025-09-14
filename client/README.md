# Rust Client Workspace

Live client to receive UDP frames from the ESP32 firmware, show a TUI, and later decode via DBC and log to disk.

## Layout
- `client-core/` — types, CSV parsing, (future) DBC decoder and state
- `client-udp/` — async UDP receiver (Tokio), parses into frames
- `client-tui/` — Ratatui app: shows raw frames and basic stats

## Build & Run
- From repo root or `client/` directory:
  - Demo (fake data): `cargo run -p client-tui -- --demo`
  - UDP (from firmware): `cargo run -p client-tui -- --host 0.0.0.0 --port 45454 --dbc ../dbc/bmw_e90.dbc`
- Quit TUI: press `q`

## Next Steps
- Load and use the DBC to decode key signals
- Add logging (CSV/JSONL) and replay
- Panels for PT-CAN and K-CAN signal groups
