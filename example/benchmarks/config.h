#pragma once

#define NUM_WARMUP 1000
#define NUM_SAMPLES 100000

/* These numbers are used as the channels from the manager POV.
 * If it is defined as 0, it is disabled.
 * If adding a new one, add it to benchmark_start_stop_channels in manager.
*/
#define BENCHMARK_CH__SIGNAL_SAME_CORE_LOW_HI 1
// #define BENCHMARK_CH__SIGNAL_SAME_CORE_LOW_HI 0

#define BENCHMARK_CH__SIGNAL_SAME_CORE_HI_LOW 2
// #define BENCHMARK_CH__SIGNAL_SAME_CORE_HI_LOW 0
