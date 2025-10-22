/*
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <stddef.h>
#include <microkit.h>
#include <assert.h>

#include "benchmark.h"
#include "config.h"

#define SIGNAL_LO_HI_CHANNEL 1

uintptr_t results;

void init(void)
{
    seL4_Word badge;
    seL4_MessageInfo_t tag UNUSED;
    cycles_t start;
    cycles_t end;

    print("hello world\n");

    /* wait for start notification */
    tag = seL4_Recv(INPUT_CAP, &badge, REPLY_CAP);

    RECORDING_BEGIN();

    for (size_t i = 0; i < NUM_WARMUP; i++) {
        start = pmu_read_cycles();
        seL4_Signal(BASE_OUTPUT_NOTIFICATION_CAP + SIGNAL_LO_HI_CHANNEL);
        end = pmu_read_cycles();

        tag = seL4_Recv(INPUT_CAP, &badge, REPLY_CAP);
        asm volatile("" :: "r"(start), "r"(end));
    }

    for (size_t i = 0; i < NUM_SAMPLES; i++) {

        /* ==== Benchmark critical ==== */
        {
            start = pmu_read_cycles();
            /* Notify low (does not switch threads) */
            seL4_Signal(BASE_OUTPUT_NOTIFICATION_CAP + SIGNAL_LO_HI_CHANNEL);
            end = pmu_read_cycles();
        }

        RECORDING_ADD_SAMPLE(start, end);

        /* Now wait, taking us out of the scheduling queue and allowing the low
           priority to run and make the notification now inactive again */
        tag = seL4_Recv(INPUT_CAP, &badge, REPLY_CAP);
    }

    RECORDING_END(results, BENCHMARK_CH__SIGNAL_SAME_CORE_HI_LOW);

    microkit_notify(BENCHMARK_START_STOP_CH);
}

DECLARE_SUBVERTED_MICROKIT()
