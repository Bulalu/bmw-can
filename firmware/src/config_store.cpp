#include "config_store.hpp"
#include <Preferences.h>

using namespace ::cfgstore;

static const char* NS = "bmwcan";

bool cfgstore::load(Config& cfg) {
  Preferences p;
  if (!p.begin(NS, /*readOnly=*/true)) return false;
  cfg.can_bps = p.getUInt("can_bps", cfg.can_bps);
  cfg.can_tx = p.getUChar("can_tx", cfg.can_tx);
  cfg.can_rx = p.getUChar("can_rx", cfg.can_rx);
  cfg.udp_port = static_cast<uint16_t>(p.getUInt("udp_port", cfg.udp_port));
  cfg.udp_host = p.getString("udp_host", cfg.udp_host.c_str());
  cfg.wifi_ssid = p.getString("wifi_ssid", cfg.wifi_ssid.c_str());
  cfg.wifi_pass = p.getString("wifi_pass", cfg.wifi_pass.c_str());
  p.end();
  return true;
}

bool cfgstore::save(const Config& cfg) {
  Preferences p;
  if (!p.begin(NS, /*readOnly=*/false)) return false;
  p.putUInt("can_bps", cfg.can_bps);
  p.putUChar("can_tx", cfg.can_tx);
  p.putUChar("can_rx", cfg.can_rx);
  p.putUInt("udp_port", cfg.udp_port);
  p.putString("udp_host", cfg.udp_host);
  p.putString("wifi_ssid", cfg.wifi_ssid);
  p.putString("wifi_pass", cfg.wifi_pass);
  p.end();
  return true;
}

