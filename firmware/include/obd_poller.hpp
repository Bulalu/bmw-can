#pragma once
#include <Arduino.h>
#include <vector>
#include <functional>
#include "can_bus.hpp"

// Minimal OBD poller for Mode 01 single-frame PIDs on standard CAN (11-bit IDs).
// Sends functional requests on 0x7DF and expects single-frame responses.
// Multi-frame (ISO-TP) not handled here.
class ObdPoller {
 public:
  void begin(uint32_t default_period_ms = 200);
  void addPid(uint8_t pid, uint32_t period_ms);
  void tick(const std::function<bool(const Frame&)>& send_fn,
            const std::function<void(const Frame&)>& on_tx_log);
  // Feed RX frames so we can handle discovery replies and ISO‑TP FF detection.
  void onRx(const Frame& f);
  // Start capability discovery (Mode 01 0x00[/0x20/...] masks) and update poll set.
  void startDiscover(const std::function<bool(const Frame&)>& send_fn,
                     const std::function<void(const Frame&)>& on_tx_log);
  // Trigger read-only DTC request (Mode 03/07/0A) via physical addressing (0x7E0).
  void requestDtc(uint8_t mode,
                  const std::function<bool(const Frame&)>& send_fn,
                  const std::function<void(const Frame&)>& on_tx_log);

 private:
  struct Item { uint8_t pid; uint32_t period_ms; unsigned long last_ms; };
  std::vector<Item> items_;
  // Capability mask (PIDs 0x01..0x80). Index is PID value.
  bool supported_[129]{}; // ignore index 0
  // Discovery state
  bool discovering_{false};
  uint8_t discover_base_{0x00};
  unsigned long discover_deadline_ms_{0};
  bool discover_next_range_{false};
  // DTC state
  bool dtc_pending_{false};
  // Paced scheduler state (one request in-flight at a time)
  size_t cursor_{0};
  bool awaiting_{false};
  uint8_t awaiting_pid_{0};
  unsigned long sent_ms_{0};
  const uint32_t timeout_ms_{250};
  const uint32_t min_gap_ms_{60};
  // Periodic DTC scheduler
  unsigned long dtc_last_ms_{0};
  const uint32_t dtc_period_ms_{5000};
  uint8_t dtc_stage_{0}; // 0=idle, 1=send 01 01, 2=send 03, 3=send 07, 4=send 0A
  bool dtc_active_{false};
  bool dtc_enabled_{false}; // disabled per user request
  // In-flight list state
  uint8_t dtc_mode_{0};        // expected response service: 0x43/0x47/0x4A
  int dtc_count_hint_{-1};     // from 41 01, -1 unknown
  uint32_t dtc_started_ms_{0};
  uint32_t dtc_last_rx_ms_{0};
  bool dtc_need_fc_{false};
  bool dtc_sent_fc_{false};
  uint32_t dtc_total_timeout_ms_{1500};
  uint32_t dtc_quiet_timeout_ms_{250};
};
