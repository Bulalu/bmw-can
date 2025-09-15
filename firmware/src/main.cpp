#include <Arduino.h>
#include "config.hpp"
#include "can_bus.hpp"
#include "net_udp.hpp"
#include "cli.hpp"
#include "config_store.hpp"
#include "selftest.hpp"

static Config g_cfg;
static CanBus g_can;
static NetUdp g_udp;
static Cli* g_cli;
static SelfTest g_self;
static bool g_raw_serial = false;

void setup() {
  Serial.begin(115200);
  delay(500);
  Serial.println();
  Serial.println("BMW-CAN ESP32 firmware scaffold");

  g_cfg = defaultConfig();
#ifndef DISABLE_NVS
  // Load persisted overrides (if present)
  cfgstore::load(g_cfg);
#else
  Serial.println("[CFG] NVS disabled at build time (DISABLE_NVS)");
#endif
  g_udp.begin(g_cfg);
  g_can.begin(g_cfg);
  g_cli = new Cli(g_cfg, g_can, g_udp, g_self, g_raw_serial);

  Serial.println("Setup complete. Type 'help' over serial.");
}

void loop() {
  if (g_cli) g_cli->tick();
  g_udp.tick();
  g_can.tick([&](const Frame& f) {
    g_udp.sendFrame(f);
    if (g_raw_serial) {
      // Print CSV to serial for wiring/bitrate debug
      Serial.printf("%llu,0x%X,%u,", static_cast<unsigned long long>(f.ts_us), f.id, f.dlc);
      static const char* hex = "0123456789ABCDEF";
      for (uint8_t i = 0; i < f.dlc && i < 8; ++i) {
        Serial.write(hex[(f.data[i] >> 4) & 0xF]);
        Serial.write(hex[f.data[i] & 0xF]);
      }
      Serial.write('\n');
    }
  });
  g_self.tick([&](const Frame& f) {
    g_udp.sendFrame(f);
    if (g_raw_serial) {
      Serial.printf("%llu,0x%X,%u,", static_cast<unsigned long long>(f.ts_us), f.id, f.dlc);
      static const char* hex = "0123456789ABCDEF";
      for (uint8_t i = 0; i < f.dlc && i < 8; ++i) {
        Serial.write(hex[(f.data[i] >> 4) & 0xF]);
        Serial.write(hex[f.data[i] & 0xF]);
      }
      Serial.write('\n');
    }
  });
  delay(10);
}
