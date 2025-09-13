#include "net_udp.hpp"

void NetUdp::begin(const Config& cfg) {
  remote_.fromString(cfg.udp_host.c_str());
  port_ = cfg.udp_port;

  WiFi.mode(WIFI_STA);
#ifdef WIFI_SSID
  Serial.printf("[WiFi] Connecting to %s...\n", WIFI_SSID);
  WiFi.begin(WIFI_SSID, WIFI_PASS);
  unsigned long start = millis();
  while (WiFi.status() != WL_CONNECTED && millis() - start < 15000) {
    delay(300);
    Serial.print(".");
  }
  Serial.println();
  if (WiFi.status() == WL_CONNECTED) {
    Serial.printf("[WiFi] Connected. IP: %s\n", WiFi.localIP().toString().c_str());
  } else {
    Serial.println("[WiFi] Not connected (timeout). Will still attempt UDP sends.");
  }
#else
  Serial.println("[WiFi] WIFI_SSID not defined. Skipping Wi-Fi connect.");
#endif
  udp_.begin(0); // random local port
  Serial.printf("[UDP] Remote %s:%u\n", cfg.udp_host.c_str(), cfg.udp_port);
}

void NetUdp::sendFrame(const Frame& f) {
  char line[96];
  // ts_us,id,dlc,data_hex
  int n = snprintf(line, sizeof(line), "%llu,0x%X,%u,",
                   static_cast<unsigned long long>(f.ts_us), f.id, f.dlc);
  static const char* hex = "0123456789ABCDEF";
  for (uint8_t i = 0; i < f.dlc && i < 8 && n + 2 < (int)sizeof(line) - 2; ++i) {
    line[n++] = hex[(f.data[i] >> 4) & 0xF];
    line[n++] = hex[f.data[i] & 0xF];
  }
  line[n++] = '\n';
  udp_.beginPacket(remote_, port_);
  udp_.write((const uint8_t*)line, n);
  udp_.endPacket();
}
