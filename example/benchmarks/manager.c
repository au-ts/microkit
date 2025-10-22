/*
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <assert.h>
#include <stddef.h>
#include <stdint.h>
#include <microkit.h>

#include "benchmark.h"
#include "config.h"
#include "kernel/gen_config.h"
#include "print.h"

uintptr_t results;

typedef struct {
    microkit_channel start_stop_ch;
    const char* name;
} benchmark_t;

static const benchmark_t benchmark_infos[] = {
    { BENCHMARK_CH__SIGNAL_LOW_HI, "Signal low to Hi" }
};

static const size_t benchmark_infos_count = sizeof(benchmark_infos)/sizeof(benchmark_infos[0]);

static_assert(sizeof(benchmark_infos)/sizeof(benchmark_infos[0]) * sizeof(result_t) <= 0x1000, "benchmark results fit");

static void start_benchmark(size_t current) {
start:
    if (current >= benchmark_infos_count) {
        print("All benchmarks done\n");
        puts("__RESULTS_BEGIN__\n");
        puts("name,runs,sum,sum_squared,min,max\n");
        for (size_t i = 0; i < benchmark_infos_count; i++) {
            const benchmark_t *info = &benchmark_infos[i];
            if (info->start_stop_ch == 0) continue;
            const result_t *result = &((const result_t *)results)[info->start_stop_ch];

            puts(info->name);
            puts(",");
            puthex64(NUM_SAMPLES);
            puts(",");
            puthex64(result->sum);
            puts(",");
            puthex64(result->sum_squared);
            puts(",");
            puthex64(result->min);
            puts(",");
            puthex64(result->max);
            puts("\n");
        }
        puts("__RESULTS_END__\n");

        puts("All is well in the universe.\n");

        return;
    }

    const benchmark_t *info = &benchmark_infos[current];
    if (info->start_stop_ch == 0) {
        current++;
        goto start;
    }

    print("Running benchmark '");
    puts(info->name);
    puts("' [");
    puthex32(current);
    puts("/");
    puthex32(benchmark_infos_count);
    puts(")\n");

    microkit_notify(info->start_stop_ch);
}

void init(void) {
    static_assert(CONFIG_EXPORT_PMU_USER);

    print("hello world\n");

    print("Available benchmarks:\n");
    for (size_t i = 0; i < benchmark_infos_count; i++) {
        const benchmark_t *info = &benchmark_infos[i];
        print("\t");
        puts(info->name);
        if (info->start_stop_ch == 0) {
            puts(" (disabled)\n");
        } else {
            puts(" (enabled)\n");
        }
    }

    print("Starting benchmark run...\n");

    start_benchmark(0);
}

void notified(microkit_channel ch) {
    print("Benchmark complete: ");
    puthex32(ch);
    puts("\n");

    start_benchmark(ch + 1);
}
