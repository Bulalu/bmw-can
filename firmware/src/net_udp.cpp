#include "net_udp.hpp"

void NetUdp::begin(const Config& cfg) {
  remote_.fromString(cfg.udp_host.c_str());
  port_ = cfg.udp_port;

  WiFi.mode(WIFI_STA);
  String ssid = cfg.wifi_ssid;
  String pass = cfg.wifi_pass;
#ifdef WIFI_SSID
  if (ssid.isEmpty()) ssid = WIFI_SSID;
#endif
#ifdef WIFI_PASS
  if (pass.isEmpty()) pass = WIFI_PASS;
#endif
  if (!ssid.isEmpty()) {
    Serial.printf("[WiFi] Connecting to %s...\n", ssid.c_str());
    WiFi.begin(ssid.c_str(), pass.c_str());
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
  } else {
    Serial.println("[WiFi] No SSID configured. Set via CLI: set wifi_ssid <ssid>; set wifi_pass <pass>; save; net reconnect");
  }
  // Listen on the same port to accept HELLO auto-sink messages
  udp_.begin(port_);
  Serial.printf("[UDP] Remote %s:%u (listening on %u)\n", cfg.udp_host.c_str(), cfg.udp_port, port_);
}

void NetUdp::sendFrame(const Frame& f) {
  if (WiFi.status() != WL_CONNECTED) return;
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

void NetUdp::tick() {
  // Auto-learn sink: expect a small ASCII packet like "HELLO 45454"
  int sz = udp_.parsePacket();
  if (sz > 0 && auto_sink_) {
    char buf[64];
    int n = udp_.read(buf, sizeof(buf) - 1);
    if (n < 0) return;
    buf[n] = '\0';
    if (strncmp(buf, "HELLO", 5) == 0) {
      uint16_t new_port = port_;
      // Parse optional port after HELLO
      char* p = buf + 5;
      while (*p == ' ') ++p;
      if (*p) {
        new_port = (uint16_t)atoi(p);
      }
      remote_ = udp_.remoteIP();
      port_ = new_port;
      last_hello_ms_ = millis();
      Serial.printf("[UDP] Sink set to %s:%u via HELLO\n", remote_.toString().c_str(), port_);
    }
  }
}
