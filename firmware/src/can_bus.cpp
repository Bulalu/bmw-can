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
                                                              TWAI_MODE_NORMAL);
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
  twai_message_t msg;
  // Non-blocking poll
  if (twai_receive(&msg, 0) == ESP_OK) {
    Frame f;
    f.id = msg.identifier;
    f.dlc = msg.data_length_code;
    for (uint8_t i = 0; i < f.dlc && i < 8; ++i) f.data[i] = msg.data[i];
    f.ts_us = micros();
    onFrame(f);
  }
}
