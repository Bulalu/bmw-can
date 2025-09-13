#include "cli.hpp"

Cli::Cli(Config& cfg, CanBus& can, NetUdp& udp)
    : cfg_(cfg), can_(can), udp_(udp) {}

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
                  cfg_.can_bps, cfg_.can_tx, cfg_.can_rx, cfg_.udp_host, cfg_.udp_port);
    return;
  }
  if (line == "help" || line == "?") {
    Serial.println("Commands: get, help");
    Serial.println("Planned: set <key> <value>, save, reboot");
    return;
  }
  Serial.println("Unknown command. Try 'help'.");
}

