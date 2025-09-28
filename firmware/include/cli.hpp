#pragma once
#include <Arduino.h>
#include "config.hpp"
#ifdef BUILD_OBD
#include <functional>
#endif
#include "can_bus.hpp"
#include "net_udp.hpp"
#include "selftest.hpp"

class Cli {
 public:
  Cli(Config& cfg, CanBus& can, NetUdp& udp, SelfTest& st, bool& rawSerialFlag);
  void tick();
  #ifdef BUILD_OBD
  void setObdDiscover(const std::function<void()>& fn) { obd_discover_cb_ = fn; }
  void setDtcRead(const std::function<void(uint8_t)>& fn) { dtc_read_cb_ = fn; }
  #endif

 private:
  Config& cfg_;
  CanBus& can_;
  NetUdp& udp_;
  SelfTest& st_;
  bool& raw_serial_;
  String buf_;
  void handleLine(const String& line);
  #ifdef BUILD_OBD
  std::function<void()> obd_discover_cb_;
  std::function<void(uint8_t)> dtc_read_cb_;
  #endif
};
