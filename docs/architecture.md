# BMW-CAN Project Overview

This document explains the architecture and intent behind the `bmw-can` codebase. It is designed to help developers onboard quickly and understand how the various components fit together.

---

## 🎯 Project Goal

The project aims to tap into a BMW E90’s PT-CAN and K-CAN lines using a custom-built ESP32-based sniffer. Raw CAN data will be forwarded over Wi-Fi to a local machine, where it will be:

1. **Decoded in real time** using a Rust-based application.
2. **Displayed live** via a Terminal User Interface (TUI) using Ratatui.
3. **Logged for offline analysis** using Python.

---

## 🗂️ Directory Structure

```bash
bmw-can/
├── firmware/       # ESP32 code for sniffing and sending CAN frames
├── client/         # Rust TUI client: receive, decode, visualize
├── analytics/      # Python scripts for offline analysis, plotting
├── docs/           # Documentation for contributors and devs
└── dbc/            # BMW CAN DBC files (e.g., bmw_e90.dbc)
```

---

## 🔌 Firmware (ESP32)

* **PlatformIO Config:**

  ```ini
  [env:esp32dev]
  platform = espressif32
  board = esp32dev
  framework = arduino
  ```

* **Hardware Configuration:**

  ```cpp
  // CAN transceiver pins
  CAN_TX_PIN = GPIO_NUM_13;
  CAN_RX_PIN = GPIO_NUM_5;
  ```

* Supports modular switching between **PT-CAN** and **K-CAN** baud rates.

  * PT-CAN is typically **500 kbps**
  * K-CAN is typically **100 kbps**
  * Use a config constant, serial command, or compile-time flag to toggle baudrates

* Uses TWAI (CAN) controller to read CAN frames and forward them over **Wi-Fi UDP** to a host machine.

* Posi-taps are used to non-invasively tap the twisted CAN pairs.

---

## 🦀 Client (Rust TUI + Decoder)

* Uses a `dbc` parser or custom decoding logic.
* Handles UDP input and updates in-memory vehicle state.
* Displays real-time dashboard using `ratatui` with panels for:

  * RPM, Speed, Throttle (PT-CAN)
  * Doors, Lights, Warnings (K-CAN)
* Optionally writes structured data to log files for later use.

---

## 📊 Analytics (Python)

* Reads dumped CAN logs from the client
* Performs post-hoc signal analysis and visualization
* Could use `pandas`, `matplotlib`, or `plotly` for insight extraction

---

## ✅ Why Rust for Client

* Zero-latency decoding and visualization
* Safe concurrency and memory usage
* One static binary for ease of deployment

Python will still be used for heavy analysis and scripting, but real-time work is done in Rust for performance and control.

---

## 🛠 Example Use

* Start ESP32 firmware to sniff CAN
* Run the Rust client to visualize live metrics
* Log output to file
* Use Python to analyze trends, drive cycles, anomalies


---

> This structure should help new devs understand what goes where and how the firmware, real-time display, and analysis layers interact.
