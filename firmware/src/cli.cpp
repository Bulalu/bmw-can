#include "cli.hpp"
#include "config_store.hpp"
#include <vector>

Cli::Cli(Config& cfg, CanBus& can, NetUdp& udp, SelfTest& st)
    : cfg_(cfg), can_(can), udp_(udp), st_(st) {}

void Cli::tick() {
  while (Serial.available()) {
    char c = static_cast<char>(Serial.read());
    if (c == '\n' || c == '\r') {
      if (buf_.length()) handleLine(buf_);
      buf_.clear();
    } else {
      buf_ += c;
    }
  }
}

void Cli::handleLine(const String& line) {
  if (line == "get") {
    Serial.printf("can_bps=%u, tx=%u, rx=%u, host=%s, port=%u\n",
                  cfg_.can_bps, cfg_.can_tx, cfg_.can_rx, cfg_.udp_host.c_str(), cfg_.udp_port);
    return;
  }
  if (line == "help" || line == "?") {
    Serial.println("Commands:");
    Serial.println("  get");
    Serial.println("  set <key> <value>   # keys: can_bps, host, port, wifi_ssid, wifi_pass");
    Serial.println("  save                # persist to NVS");
    Serial.println("  net reconnect       # reapply Wi‑Fi/UDP without reboot");
    Serial.println("  reboot");
    Serial.println("  selftest on|off     # generate synthetic frames");
    Serial.println("  selftest once <n>   # emit N frames immediately");
    return;
  }
  if (line == "save") {
    if (cfgstore::save(cfg_)) Serial.println("Saved to NVS.");
    else Serial.println("Save failed.");
    return;
  }
  if (line == "reboot") {
    Serial.println("Rebooting...");
    delay(50);
    ESP.restart();
    return;
  }
  if (line == "net reconnect") {
    udp_.begin(cfg_);
    Serial.println("Network reconfigured.");
    return;
  }

  if (line.startsWith("selftest ")) {
    if (line.endsWith(" on") || line.endsWith(" on\r") || line.endsWith(" on\n")) {
      st_.setEnabled(true);
      Serial.println("selftest enabled");
      return;
    }
    if (line.endsWith(" off") || line.endsWith(" off\r") || line.endsWith(" off\n")) {
      st_.setEnabled(false);
      Serial.println("selftest disabled");
      return;
    }
    int sp2 = line.indexOf(' ', line.indexOf(' ') + 1);
    String sub = line.substring(line.indexOf(' ') + 1, sp2);
    if (sub == "once") {
      String nStr = line.substring(sp2 + 1);
      size_t n = (size_t)nStr.toInt();
      if (n == 0) n = 1;
      st_.emitOnce(n, [&](const Frame& f){ udp_.sendFrame(f); });
      Serial.printf("emitted %u frames\n", (unsigned)n);
      return;
    }
    Serial.println("Usage: selftest on|off | selftest once <n>");
    return;
  }

  if (line.startsWith("set ")) {
    // tokenize: set <key> <value...>
    int firstSpace = line.indexOf(' ');
    int secondSpace = line.indexOf(' ', firstSpace + 1);
    if (secondSpace < 0) {
      Serial.println("Usage: set <key> <value>");
      return;
    }
    String key = line.substring(firstSpace + 1, secondSpace);
    String value = line.substring(secondSpace + 1);
    key.toLowerCase();
    if (key == "can_bps") {
      cfg_.can_bps = value.toInt();
      Serial.printf("can_bps=%u\n", cfg_.can_bps);
    } else if (key == "host") {
      cfg_.udp_host = value;
      Serial.printf("host=%s\n", cfg_.udp_host.c_str());
    } else if (key == "port") {
      cfg_.udp_port = static_cast<uint16_t>(value.toInt());
      Serial.printf("port=%u\n", cfg_.udp_port);
    } else if (key == "wifi_ssid") {
      cfg_.wifi_ssid = value;
      Serial.println("wifi_ssid set.");
    } else if (key == "wifi_pass") {
      cfg_.wifi_pass = value;
      Serial.println("wifi_pass set.");
    } else {
      Serial.println("Unknown key.");
    }
    return;
  }
  Serial.println("Unknown command. Try 'help'.");
}
