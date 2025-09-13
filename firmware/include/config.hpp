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
#define UDP_HOST "192.168.1.5"
#endif

struct Config {
  uint32_t can_bps;
  uint8_t can_tx;
  uint8_t can_rx;
  String udp_host;
  uint16_t udp_port;
  String wifi_ssid;
  String wifi_pass;
};

inline Config defaultConfig() {
  Config c;
  c.can_bps = CAN_BPS;
  c.can_tx = CAN_TX;
  c.can_rx = CAN_RX;
  c.udp_host = String(UDP_HOST);
  c.udp_port = UDP_PORT;
#ifdef WIFI_SSID
  c.wifi_ssid = String(WIFI_SSID);
#endif
#ifdef WIFI_PASS
  c.wifi_pass = String(WIFI_PASS);
#endif
  return c;
}
