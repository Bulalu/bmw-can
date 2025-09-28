#include "obd_poller.hpp"
#include <Arduino.h>

void ObdPoller::begin(uint32_t default_period_ms) {
  items_.clear();
  // Fixed, confirmed supported PIDs for this car (no discovery)
  addPid(0x0C, default_period_ms); // RPM
  addPid(0x0D, default_period_ms); // Speed
  addPid(0x05, default_period_ms); // Coolant
  addPid(0x2F, 1000);              // Fuel level
  addPid(0x42, 1000);              // Module voltage
  // DTC status/count intentionally omitted for now
  addPid(0x04, 400);               // Engine load
  addPid(0x11, 400);               // Throttle
  addPid(0x0F, 1000);              // Intake air temp
  addPid(0x06, 800);               // STFT B1
  addPid(0x07, 1500);              // LTFT B1
  addPid(0x33, 1500);              // Barometric pressure
  addPid(0x1F, 2000);              // Time since engine start
}

void ObdPoller::addPid(uint8_t pid, uint32_t period_ms) {
  items_.push_back(Item{pid, period_ms, 0});
}

void ObdPoller::tick(const std::function<bool(const Frame&)>& send_fn,
                     const std::function<void(const Frame&)>& on_tx_log) {
  const unsigned long now = millis();
  // Discovery progression windowing (not used unless started manually)
  if (discovering_ && now >= discover_deadline_ms_) {
    if (discover_next_range_) {
      discover_base_ = static_cast<uint8_t>(discover_base_ + 0x20);
      Frame f{}; f.id=0x7DF; f.dlc=8; f.data[0]=0x02; f.data[1]=0x01; f.data[2]=discover_base_; for (int i=3;i<8;++i) f.data[i]=0x00; f.ts_us=micros();
      if (send_fn(f)) on_tx_log(f);
      discover_next_range_ = false;
      discover_deadline_ms_ = now + 200;
    } else {
      // Completed discovery: filter polling list to supported PIDs
      std::vector<Item> kept; kept.reserve(items_.size());
      for (auto &it : items_) { if (it.pid <= 0x80 && supported_[it.pid]) kept.push_back(it); }
      items_.swap(kept);
      discovering_ = false;
      // Print a brief summary to serial for visibility
      Serial.print("[OBD] discover complete. Polling PIDs:");
      for (auto &it : items_) { Serial.printf(" 0x%02X", it.pid); }
      Serial.println();
    }
  }
  // Paced, single in-flight PID request logic
  // Before normal polling, handle periodic DTC sequence
  // Pause normal PID polling while a DTC stage is active
  if (dtc_enabled_ && dtc_stage_ == 0) {
    if (!awaiting_ && (dtc_last_ms_ == 0 || now - dtc_last_ms_ >= dtc_period_ms_)) {
      dtc_stage_ = 1; dtc_active_ = true; dtc_count_hint_ = -1; dtc_mode_ = 0; dtc_need_fc_ = false; dtc_sent_fc_ = false;
    }
  }
  if (dtc_enabled_ && dtc_stage_ >= 1) {
    // Send FC if needed (after detecting FF)
    if (dtc_need_fc_ && !dtc_sent_fc_ && (now - sent_ms_ >= min_gap_ms_)) {
      Frame fc{}; fc.id=0x7E0; fc.dlc=8; fc.data[0]=0x30; fc.data[1]=0x00; fc.data[2]=0x00; for(int i=3;i<8;++i) fc.data[i]=0x00; fc.ts_us=micros();
      if (send_fn(fc)) { on_tx_log(fc); dtc_sent_fc_ = true; sent_ms_ = now; }
    }
    // Handle stage progression
    if (dtc_stage_ == 1 && (now - sent_ms_ >= min_gap_ms_)) {
      // Mode 01 PID 01 (status/count)
      Frame s{}; s.id=0x7DF; s.dlc=8; s.data[0]=0x02; s.data[1]=0x01; s.data[2]=0x01; for(int i=3;i<8;++i) s.data[i]=0x00; s.ts_us=micros();
      if (send_fn(s)) { on_tx_log(s); sent_ms_=now; dtc_started_ms_=now; dtc_stage_ = 2; }
    } else if (dtc_stage_ >= 2) {
      // If count hint known and zero, skip lists
      if (dtc_count_hint_ == 0) {
        dtc_stage_ = 0; dtc_active_ = false; dtc_last_ms_ = now;
      } else if (dtc_count_hint_ != -1) {
        // Send list requests one by one with time windows
        if (dtc_mode_ == 0 && (now - sent_ms_ >= min_gap_ms_)) {
          // Issue Mode 03 (stored)
          Frame req{}; req.id=0x7E0; req.dlc=8; req.data[0]=0x02; req.data[1]=0x03; req.data[2]=0x00; for(int i=3;i<8;++i) req.data[i]=0x00; req.ts_us=micros();
          if (send_fn(req)) { on_tx_log(req); sent_ms_=now; dtc_mode_=0x43; dtc_started_ms_=now; dtc_last_rx_ms_=now; dtc_need_fc_=false; dtc_sent_fc_=false; }
        }
        // Advance to next list if timeout/quiet elapsed and a mode was set
        if (dtc_mode_ == 0x43) {
          if ((now - dtc_started_ms_ > dtc_total_timeout_ms_) || (dtc_sent_fc_ && (now - dtc_last_rx_ms_ > dtc_quiet_timeout_ms_))) {
            dtc_mode_ = 0; // move on to next
            // Next: Mode 07
            Frame req{}; req.id=0x7E0; req.dlc=8; req.data[0]=0x02; req.data[1]=0x07; req.data[2]=0x00; for(int i=3;i<8;++i) req.data[i]=0x00; req.ts_us=micros();
            if (send_fn(req)) { on_tx_log(req); sent_ms_=now; dtc_mode_=0x47; dtc_started_ms_=now; dtc_last_rx_ms_=now; dtc_need_fc_=false; dtc_sent_fc_=false; }
          }
        } else if (dtc_mode_ == 0x47) {
          if ((now - dtc_started_ms_ > dtc_total_timeout_ms_) || (dtc_sent_fc_ && (now - dtc_last_rx_ms_ > dtc_quiet_timeout_ms_))) {
            dtc_mode_ = 0; // next: Mode 0A
            Frame req{}; req.id=0x7E0; req.dlc=8; req.data[0]=0x02; req.data[1]=0x0A; req.data[2]=0x00; for(int i=3;i<8;++i) req.data[i]=0x00; req.ts_us=micros();
            if (send_fn(req)) { on_tx_log(req); sent_ms_=now; dtc_mode_=0x4A; dtc_started_ms_=now; dtc_last_rx_ms_=now; dtc_need_fc_=false; dtc_sent_fc_=false; }
          }
        } else if (dtc_mode_ == 0x4A) {
          if ((now - dtc_started_ms_ > dtc_total_timeout_ms_) || (dtc_sent_fc_ && (now - dtc_last_rx_ms_ > dtc_quiet_timeout_ms_))) {
            // Done
            dtc_mode_ = 0; dtc_stage_ = 0; dtc_active_ = false; dtc_last_ms_ = now;
          }
        }
      }
    }
  }

  // Paced, single in-flight PID request logic (skip while DTC active)
  if ((!dtc_enabled_ || !dtc_active_) && !items_.empty()) {
    if (awaiting_) {
      // Timeout advance if no reply
      if (now - sent_ms_ >= timeout_ms_) {
        awaiting_ = false;
        cursor_ = (cursor_ + 1) % items_.size();
      }
    } else {
      Item &it = items_[cursor_];
      if (it.last_ms == 0 || now - it.last_ms >= it.period_ms) {
        // Respect minimal gap since previous send
        if (now - sent_ms_ >= min_gap_ms_) {
          Frame f{}; f.id=0x7DF; f.dlc=8; f.data[0]=0x02; f.data[1]=0x01; f.data[2]=it.pid; for (int i=3;i<8;++i) f.data[i]=0x00; f.ts_us=micros();
          if (send_fn(f)) { on_tx_log(f); awaiting_=true; awaiting_pid_=it.pid; sent_ms_=now; it.last_ms=now; }
        }
      } else {
        // Not due yet; move cursor to next candidate to keep loop responsive
        cursor_ = (cursor_ + 1) % items_.size();
      }
    }
  }
}

void ObdPoller::onRx(const Frame& f) {
  // Capability discovery replies (kept for optional manual use)
  if (discovering_ && f.dlc >= 6) {
    if ((f.id & 0x7F0) == 0x7E0) {
      // Expect 41 <base> <A> <B> <C> <D>
      uint8_t pci_type = f.data[0] & 0xF0; // 0x00=SF, 0x10=FF
      if (pci_type == 0x00 || pci_type == 0x10) {
        if (f.data[1] == 0x41 && f.data[2] == discover_base_) {
          if (f.dlc >= 7) {
            uint8_t A=f.data[3], B=f.data[4], C=f.data[5], D=f.data[6];
            for (int i=0;i<32;++i) {
              bool bit=false;
              if (i<8) bit=(A & (1<<(7-i)))!=0; else if(i<16) bit=(B & (1<<(15-i)))!=0; else if(i<24) bit=(C & (1<<(23-i)))!=0; else bit=(D & (1<<(31-i)))!=0;
              uint8_t pid = static_cast<uint8_t>(discover_base_ + 1 + i);
              if (pid <= 0x80) supported_[pid] = supported_[pid] || bit;
            }
            // If PID base+32 bit is set, there is a next block
            discover_next_range_ = discover_next_range_ || ((D & 0x01) != 0);
          }
        }
      }
    }
  }

  // Match in-flight PID responses to advance immediately
  if (awaiting_ && f.dlc >= 3) {
    if ((f.id & 0x7F0) == 0x7E0) {
      uint8_t pci_type = f.data[0] & 0xF0; // 0x00=SF
      if (pci_type == 0x00 && f.data[1] == 0x41 && f.data[2] == awaiting_pid_) {
        awaiting_ = false;
        cursor_ = (cursor_ + 1) % items_.size();
      }
    }
  }

  // DTC handling
  if (dtc_stage_ >= 1) {
    // If we are awaiting 41 01
    if (dtc_stage_ == 2 && f.dlc >= 4) {
      if ((f.id & 0x7F0) == 0x7E0) {
        uint8_t pci_type = f.data[0] & 0xF0;
        if (pci_type == 0x00 && f.data[1] == 0x41 && f.data[2] == 0x01) {
          int count = f.data[3] & 0x7F;
          dtc_count_hint_ = count;
          // If no codes at all, end sequence soon (handled in tick)
        }
      }
    }
    // If we are waiting for a list (mode in dtc_mode_)
    if ((f.id & 0x7F0) == 0x7E0 && f.dlc >= 3 && dtc_mode_ != 0) {
      uint8_t pci_type = f.data[0] & 0xF0;
      uint8_t service = 0;
      if (pci_type == 0x00) {
        service = f.data[1];
        if (service == dtc_mode_) {
          // Single frame list received; mark quiet time for advance
          dtc_last_rx_ms_ = millis();
          // No FC needed for SF; we will time out quickly to advance
        }
      } else if (pci_type == 0x10) {
        // First frame; extract expected service at byte 2
        service = f.data[2];
        if (service == dtc_mode_) {
          // Request FC to continue
          dtc_need_fc_ = true;
          dtc_last_rx_ms_ = millis();
        }
      } else if ((pci_type & 0xF0) == 0x20) {
        // Consecutive frame
        dtc_last_rx_ms_ = millis();
      }
    }
  }
}

void ObdPoller::startDiscover(const std::function<bool(const Frame&)>& send_fn,
                              const std::function<void(const Frame&)>& on_tx_log) {
  for (int i=0;i<=0x80;++i) supported_[i]=false;
  discovering_ = true;
  discover_base_ = 0x00;
  discover_next_range_ = false;
  Frame f{}; f.id=0x7DF; f.dlc=8; f.data[0]=0x02; f.data[1]=0x01; f.data[2]=discover_base_; for (int i=3;i<8;++i) f.data[i]=0x00; f.ts_us=micros();
  if (send_fn(f)) on_tx_log(f);
  discover_deadline_ms_ = millis() + 200;
}

void ObdPoller::requestDtc(uint8_t mode,
                           const std::function<bool(const Frame&)>& send_fn,
                           const std::function<void(const Frame&)>& on_tx_log) {
  // Kept for CLI/manual use; prefer periodic flow in tick
  Frame req{}; req.id=0x7E0; req.dlc=8; req.data[0]=0x02; req.data[1]=mode; req.data[2]=0x00; for (int i=3;i<8;++i) req.data[i]=0x00; req.ts_us=micros();
  if (send_fn(req)) on_tx_log(req);
}
