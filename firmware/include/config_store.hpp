#pragma once
#include <Arduino.h>
#include "config.hpp"

namespace cfgstore {
  bool load(Config& cfg);
  bool save(const Config& cfg);
}

