#pragma once
#include <Arduino.h>
#include <WiFi.h>
#include <WiFiUdp.h>
#include "config.hpp"
#include "can_bus.hpp"

class NetUdp {
 public:
  void begin(const Config& cfg);
  void sendFrame(const Frame& f);
  void reconfigure(const Config& cfg) { begin(cfg); }

 private:
  IPAddress remote_;
  uint16_t port_ = 0;
  WiFiUDP udp_;
};
