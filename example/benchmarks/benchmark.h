#pragma once

#include <microkit.h>
#include <stdint.h>

#include "config.h"
#include "pmu.h"
#include "print.h"

#define UNUSED __attribute__((unused))

/* Because we deliberately subvert libmicrokit in these examples */
#define INPUT_CAP 1
#define REPLY_CAP 4
#define DECLARE_SUBVERTED_MICROKIT()                                           \
  void notified(microkit_channel ch) {}

/* Inside a benchmark, the start-stop ch */
#define BENCHMARK_START_STOP_CH 0

/* Declare and init internal parameters: "sample" and "is_counted" */
#define RECORDING_BEGIN()                                                      \
  pmu_enable();                                                                \
  print("BEGIN\n");                                                            \
  cycles_t sample;                                                             \
  uint64_t sum = 0;                                                            \
  uint64_t sum_squared = 0;                                                    \
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

#define RECORDING_END()                                                        \
  print("RESULTS\n");                                                          \
  puts("runs,sum,sum_squared,min,max\n");                                      \
  puthex64(NUM_SAMPLES);                                                       \
  puts(",");                                                                   \
  puthex64(sum);                                                               \
  puts(",");                                                                   \
  puthex64(sum_squared);                                                       \
  puts(",");                                                                   \
  puthex64(min);                                                               \
  puts(",");                                                                   \
  puthex64(max);                                                               \
  puts("\n");
