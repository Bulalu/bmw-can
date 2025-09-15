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
  void tick();
  void reconfigure(const Config& cfg) { begin(cfg); }
  void setAutoSink(bool on) { auto_sink_ = on; }
  bool autoSink() const { return auto_sink_; }
  IPAddress remote() const { return remote_; }
  uint16_t remotePort() const { return port_; }

 private:
  IPAddress remote_;
  uint16_t port_ = 0;
  WiFiUDP udp_;
  bool auto_sink_ = true;
  unsigned long last_hello_ms_ = 0;
};
