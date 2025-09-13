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

void setup() {
  Serial.begin(115200);
  delay(500);
  Serial.println();
  Serial.println("BMW-CAN ESP32 firmware scaffold");

  g_cfg = defaultConfig();
  // Load persisted overrides (if present)
  cfgstore::load(g_cfg);
  g_udp.begin(g_cfg);
  g_can.begin(g_cfg);
  g_cli = new Cli(g_cfg, g_can, g_udp, g_self);

  Serial.println("Setup complete. Type 'help' over serial.");
}

void loop() {
  if (g_cli) g_cli->tick();
  g_can.tick([&](const Frame& f) {
    g_udp.sendFrame(f);
  });
  g_self.tick([&](const Frame& f) {
    g_udp.sendFrame(f);
  });
  delay(10);
}
