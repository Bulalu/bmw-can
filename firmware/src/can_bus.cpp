#include "can_bus.hpp"
#include <driver/twai.h>

static twai_timing_config_t timing_from_bps(uint32_t bps) {
  switch (bps) {
    case 100000: return TWAI_TIMING_CONFIG_100KBITS();
    case 125000: return TWAI_TIMING_CONFIG_125KBITS();
    case 250000: return TWAI_TIMING_CONFIG_250KBITS();
    case 500000: return TWAI_TIMING_CONFIG_500KBITS();
    case 800000: return TWAI_TIMING_CONFIG_800KBITS();
    case 1000000: return TWAI_TIMING_CONFIG_1MBITS();
    default: return TWAI_TIMING_CONFIG_500KBITS();
  }
}

void CanBus::begin(const Config& cfg) {
  if (started_) return;

  twai_general_config_t g_config = TWAI_GENERAL_CONFIG_DEFAULT((gpio_num_t)cfg.can_tx,
                                                              (gpio_num_t)cfg.can_rx,
#ifdef BUILD_OBD
                                                              TWAI_MODE_NORMAL
#else
                                                              TWAI_MODE_LISTEN_ONLY
#endif
                                                              );
  // Increase RX queue length a bit for bursty traffic
  g_config.rx_queue_len = 32;
  g_config.tx_queue_len = 8;

  twai_timing_config_t t_config = timing_from_bps(cfg.can_bps);
  twai_filter_config_t f_config = TWAI_FILTER_CONFIG_ACCEPT_ALL();

  if (twai_driver_install(&g_config, &t_config, &f_config) != ESP_OK) {
    Serial.println("[CAN] driver_install failed");
    return;
  }
  if (twai_start() != ESP_OK) {
    Serial.println("[CAN] start failed");
    return;
  }
  started_ = true;
  Serial.printf("[CAN] started bps=%u tx=%u rx=%u\n", cfg.can_bps, cfg.can_tx, cfg.can_rx);
}

void CanBus::tick(const std::function<void(const Frame&)>& onFrame) {
  if (!started_) return;

  // Drain RX queue without blocking; no filtering (ACCEPT_ALL)
  for (;;) {
    twai_message_t msg;
    if (twai_receive(&msg, 0) != ESP_OK) break;
    Frame f;
    f.id = msg.identifier;
    f.dlc = msg.data_length_code;
    for (uint8_t i = 0; i < f.dlc && i < 8; ++i) f.data[i] = msg.data[i];
    f.ts_us = micros();
    onFrame(f);
  }

  // Lightweight status log on bus errors (throttled)
  static unsigned long last_status_ms = 0;
  unsigned long now = millis();
  if (now - last_status_ms > 1000) {
    twai_status_info_t st;
    if (twai_get_status_info(&st) == ESP_OK) {
      if (st.state == TWAI_STATE_BUS_OFF || st.rx_missed_count > 0 || st.rx_overrun_count > 0) {
        Serial.printf("[CAN] state=%d rx=%u rx_missed=%u rx_overrun=%u bus_err=%u\n",
                      (int)st.state, st.msgs_to_rx, st.rx_missed_count, st.rx_overrun_count, st.bus_error_count);
      }
    }
    last_status_ms = now;
  }
}

bool CanBus::send(const Frame& f) {
#ifdef BUILD_OBD
  if (!started_) return false;
  twai_message_t msg{};
  msg.identifier = f.id & 0x7FF;
  msg.extd = 0; // standard frame
  msg.rtr = 0;
  msg.data_length_code = f.dlc <= 8 ? f.dlc : 8;
  for (uint8_t i = 0; i < msg.data_length_code; ++i) msg.data[i] = f.data[i];
  // Non-blocking transmit
  return twai_transmit(&msg, 0) == ESP_OK;
#else
  (void)f;
  return false;
#endif
}
