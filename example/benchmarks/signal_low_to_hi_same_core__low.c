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

uintptr_t shared;
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

    for (size_t i = 0; i < NUM_WARMUP; i++) {
        start = pmu_read_cycles();
        seL4_Signal(BASE_OUTPUT_NOTIFICATION_CAP + SIGNAL_LO_HI_CHANNEL);
        tag = seL4_Recv(INPUT_CAP, &badge, REPLY_CAP);
        end = *(volatile cycles_t *)(shared);

        asm volatile("" :: "r"(start), "r"(end));
    }

    RECORDING_BEGIN();

    for (size_t i = 0; i < NUM_SAMPLES; i++) {

        /* ==== Benchmark critical ==== */
        {
            start = pmu_read_cycles();
            /* Transfer to high */
            seL4_Signal(BASE_OUTPUT_NOTIFICATION_CAP + SIGNAL_LO_HI_CHANNEL);
        }

        /* Now we wait for a reply for the higher priority telling it that it
           has updated the shared information, and record the difference */
        tag = seL4_Recv(INPUT_CAP, &badge, REPLY_CAP);

        /*
         * ARM guarantees that the writes are coherent w.r.t the same
         * physical addresses, i.e. that loads from `shared` following a
         * "program-order" store to `shared` sees the same value.
         * Since "program-order" is necessarily consistent on the same core,
         * reading `shared` will read the last written value by the HighPrio
         * PD without need for memory barriers or cache management.
         * Note that this is only for the "same observer", i.e. within the same
         * PE (CPU) or peripheral.
         *
         * Ref: ARM ARM DDII 0487 L.b, p. G5-11701, §G5.10.1 Data and unified caches
         */
        static_assert(CONFIG_ARCH_ARM, "on an ARM platform");
        end = *(volatile cycles_t *)(shared);

        RECORDING_ADD_SAMPLE(start, end);
    }

    RECORDING_END(results, BENCHMARK_CH__SIGNAL_SAME_CORE_LOW_HI);

    microkit_notify(BENCHMARK_START_STOP_CH);
}

DECLARE_SUBVERTED_MICROKIT()
