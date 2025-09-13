#include "can_bus.hpp"

void CanBus::begin(const Config&) {
  // TODO: Initialize TWAI (CAN) using cfg. Scaffold only.
}

void CanBus::tick(const std::function<void(const Frame&)>& onFrame) {
  // TODO: Read frames from CAN and invoke callback. Scaffold only.
  (void)onFrame;
}

