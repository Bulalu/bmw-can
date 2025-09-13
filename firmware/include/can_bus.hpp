#pragma once
#include <Arduino.h>
#include <functional>
#include "config.hpp"

struct Frame {
  uint32_t id{0};
  uint8_t dlc{0};
  uint8_t data[8]{};
  uint64_t ts_us{0};
};

class CanBus {
 public:
  void begin(const Config& cfg);
  // Poll for frames; call onFrame for each received frame.
  void tick(const std::function<void(const Frame&)>& onFrame);
};

