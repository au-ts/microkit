#pragma once

#include <stdint.h>

#include "config.h"
#include "print.h"

/* Because we deliberately subvert libmicrokit in these examples */
#define INPUT_CAP 1
#define REPLY_CAP 4
#define DECLARE_SUBVERTED_MICROKIT()                                           \
  void notified(microkit_channel ch) {}

/* Inside a benchmark, the start-stop ch */
#define BENCHMARK_START_STOP_CH 0

typedef uint64_t cycles_t;
#define CYCLES_MAX UINT64_MAX
#define CYCLES_MIN 0

/* Declare and init internal parameters: "sample" and "is_counted" */
#define RECORDING_BEGIN()                                                      \
  cycles_t sample;                                                             \
  uint64_t sum;                                                                \
  uint64_t sum_squared;                                                        \
  cycles_t min = CYCLES_MAX;                                                   \
  cycles_t max = CYCLES_MIN;

#define RECORDING_ADD_SAMPLE(start, end)                                       \
  /* don't let the compiler reorder these before into the benchmark */         \
  asm volatile("" ::: "memory");                                               \
  sample = (end - start);                                                      \
  sum += sample;                                                               \
  sum_squared += sample * sample;                                              \
  min = (sample < min) ? sample : min;                                         \
  max = (sample > max) ? sample : max;
