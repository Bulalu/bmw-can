#pragma once
#include <Arduino.h>
#include <functional>
#include "can_bus.hpp"

class SelfTest {
 public:
  void setEnabled(bool en) { enabled_ = en; }
  bool enabled() const { return enabled_; }
  void setRateHz(uint16_t hz) { rate_hz_ = hz ? hz : 1; }
  uint16_t rateHz() const { return rate_hz_; }

  void tick(const std::function<void(const Frame&)>& onFrame) {
    if (!enabled_) return;
    unsigned long now = micros();
    const unsigned long interval = 1000000UL / rate_hz_;
    if (now - last_us_ < interval) return;
    last_us_ = now;

    Frame f;
    f.ts_us = now;
    f.id = 0x123; // arbitrary test ID
    f.dlc = 8;
    for (uint8_t i = 0; i < 8; ++i) {
      f.data[i] = (uint8_t)(counter_ + i);
    }
    counter_++;
    onFrame(f);
  }

  void emitOnce(size_t n, const std::function<void(const Frame&)>& onFrame) {
    for (size_t k = 0; k < n; ++k) {
      Frame f;
      f.ts_us = micros();
      f.id = 0x124; // second test ID for burst
      f.dlc = 8;
      for (uint8_t i = 0; i < 8; ++i) {
        f.data[i] = (uint8_t)(counter_ + i);
      }
      counter_++;
      onFrame(f);
      delay(1);
    }
  }

 private:
  bool enabled_{false};
  uint16_t rate_hz_{20};
  unsigned long last_us_{0};
  uint8_t counter_{0};
};

