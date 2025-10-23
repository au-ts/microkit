/*
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <stdbool.h>
#include <microkit.h>

#include "benchmark.h"

#define SIGNAL_LOW_MID_CHANNEL  1
#define SIGNAL_MID_HIGH_CHANNEL 2
#define SIGNAL_HIGH_LOW_CHANNEL 3

uintptr_t shared;

void init(void)
{
    print("hello world\n");

    seL4_Word badge;
    seL4_MessageInfo_t tag UNUSED;

    while (true) {
        cycles_t end = 0;

        /* ==== Benchmark critical ==== */
        {
            /* Wait for low */
            tag = seL4_Recv(INPUT_CAP, &badge, REPLY_CAP);
            end = pmu_read_cycles();
        }

        *(volatile cycles_t *)(shared) = end;
        seL4_Signal(BASE_OUTPUT_NOTIFICATION_CAP + SIGNAL_HIGH_LOW_CHANNEL);
    }
}

DECLARE_SUBVERTED_MICROKIT()
