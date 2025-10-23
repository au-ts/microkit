/*
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <stdbool.h>
#include <microkit.h>

#include "benchmark.h"

#define SIGNAL_LO_HI_CHANNEL 1

uintptr_t shared;

void init(void)
{
    print("hello world\n");

    seL4_Word badge;
    seL4_MessageInfo_t tag UNUSED;

    /* To make this simpler this literally just always replies */
    while (true) {
        cycles_t end = 0;

        /* ==== Benchmark critical ==== */
        {
            /* Wait for low */
            tag = seL4_Recv(INPUT_CAP, &badge, REPLY_CAP);
            end = pmu_read_cycles();
        }

        *(volatile cycles_t *)(shared) = end;
        seL4_Signal(BASE_OUTPUT_NOTIFICATION_CAP + SIGNAL_LO_HI_CHANNEL);
    }
}

DECLARE_SUBVERTED_MICROKIT()
