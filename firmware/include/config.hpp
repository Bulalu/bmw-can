#pragma once
#include <Arduino.h>

#ifndef CAN_BPS
#define CAN_BPS 500000
#endif

#ifndef CAN_TX
#define CAN_TX 13
#endif

#ifndef CAN_RX
#define CAN_RX 5
#endif

#ifndef UDP_PORT
#define UDP_PORT 45454
#endif

#ifndef UDP_HOST
#define UDP_HOST "192.168.1.100"
#endif

struct Config {
  uint32_t can_bps;
  uint8_t can_tx;
  uint8_t can_rx;
  const char* udp_host;
  uint16_t udp_port;
};

inline Config defaultConfig() {
  return Config{CAN_BPS, CAN_TX, CAN_RX, UDP_HOST, UDP_PORT};
}

