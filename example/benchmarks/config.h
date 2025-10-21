#pragma once

#define NUM_WARMUP 100
#define NUM_COOLDOWN 100
#define NUM_SAMPLES 10000

/* These numbers are used as the channels from the manager POV.
 * If it is defined as 0, it is disabled.
 * If adding a new one, add it to benchmark_start_stop_channels in manager.
*/
#define BENCHMARK_CH__SIGNAL_LOW_HI 1
// #define BENCHMARK_CH__SIGNAL_LOW_HI 0

