#include "cli.hpp"
#include "config_store.hpp"
#include <driver/twai.h>
#include <vector>

Cli::Cli(Config& cfg, CanBus& can, NetUdp& udp, SelfTest& st, bool& rawSerialFlag)
    : cfg_(cfg), can_(can), udp_(udp), st_(st), raw_serial_(rawSerialFlag) {}

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
#ifdef DISABLE_NVS
    Serial.println("  save                # (disabled in this build)");
    #else
    Serial.println("  save                # persist to NVS");
    #endif
    Serial.println("  net reconnect       # reapply Wi‑Fi/UDP without reboot");
    Serial.println("  net status          # show Wi‑Fi + UDP sink info");
    Serial.println("  net auto on|off     # auto-learn sink via HELLO");
    Serial.println("  net sink <ip> <port># set sink manually");
    Serial.println("  can status          # print TWAI status once");
    Serial.println("  reboot");
    Serial.println("  selftest on|off     # generate synthetic frames");
    Serial.println("  selftest once <n>   # emit N frames immediately");
    Serial.println("  raw on|off          # print raw frames over serial");
    return;
  }
  if (line == "save") {
    #ifdef DISABLE_NVS
    Serial.println("NVS disabled in this build. Skipping save.");
    #else
    if (cfgstore::save(cfg_)) Serial.println("Saved to NVS.");
    else Serial.println("Save failed.");
    #endif
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

  if (line == "net status") {
    Serial.printf("WiFi: %s  IP: %s\n", WiFi.status() == WL_CONNECTED ? "connected" : "not connected", WiFi.localIP().toString().c_str());
    Serial.printf("UDP sink: %s:%u  auto=%s\n", udp_.remote().toString().c_str(), udp_.remotePort(), udp_.autoSink() ? "on" : "off");
    return;
  }

  if (line == "net auto on") { udp_.setAutoSink(true); Serial.println("auto-sink ON"); return; }
  if (line == "net auto off") { udp_.setAutoSink(false); Serial.println("auto-sink OFF"); return; }

  if (line.startsWith("net sink ")) {
    // net sink <ip> <port>
    int sp1 = line.indexOf(' '); int sp2 = line.indexOf(' ', sp1 + 1); int sp3 = line.indexOf(' ', sp2 + 1);
    if (sp2 < 0 || sp3 < 0) { Serial.println("Usage: net sink <ip> <port>"); return; }
    String ip = line.substring(sp2 + 1, sp3);
    String port = line.substring(sp3 + 1);
    cfg_.udp_host = ip; cfg_.udp_port = (uint16_t)port.toInt();
    udp_.begin(cfg_);
    Serial.printf("sink set to %s:%u\n", cfg_.udp_host.c_str(), cfg_.udp_port);
    return;
  }

  if (line == "can status") {
    twai_status_info_t st;
    if (twai_get_status_info(&st) == ESP_OK) {
      Serial.printf("state=%d msgs_to_rx=%u rx_missed=%u rx_overrun=%u bus_err=%u tx_err=%u rx_err=%u\n",
                    (int)st.state, st.msgs_to_rx, st.rx_missed_count, st.rx_overrun_count,
                    st.bus_error_count, st.tx_error_counter, st.rx_error_counter);
    } else {
      Serial.println("CAN status read failed");
    }
    return;
  }

  if (line == "raw on") {
    raw_serial_ = true;
    Serial.println("raw serial ON");
    return;
  }
  if (line == "raw off") {
    raw_serial_ = false;
    Serial.println("raw serial OFF");
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
