#pragma once
#include <Arduino.h>
#include "config.hpp"
#include "can_bus.hpp"
#include "net_udp.hpp"
#include "selftest.hpp"

class Cli {
 public:
  Cli(Config& cfg, CanBus& can, NetUdp& udp, SelfTest& st);
  void tick();

 private:
  Config& cfg_;
  CanBus& can_;
  NetUdp& udp_;
  SelfTest& st_;
  String buf_;
  void handleLine(const String& line);
};
