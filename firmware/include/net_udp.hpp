#pragma once
#include <Arduino.h>
#include "config.hpp"
#include "can_bus.hpp"

class NetUdp {
 public:
  void begin(const Config& cfg);
  void sendFrame(const Frame& f);
};

