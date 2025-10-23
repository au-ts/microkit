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

typedef struct {
  uint64_t sum;
  uint64_t sum_squared;
  cycles_t min;
  cycles_t max;
} result_t;

#define RECORDING_END(results_ptr)                              \
  do {                                                                         \
    /* TODO: cache flushes for multicore? */                                   \
    print("END\n");                                                            \
    result_t *_results = (void *)results_ptr;                \
    _results->sum = sum;                                         \
    _results->sum_squared = sum_squared;                         \
    _results->min = min;                                         \
    _results->max = max;                                         \
  } while (0)
