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

void init(void)
{
    print("hello world\n");

    seL4_Word badge;
    seL4_MessageInfo_t tag UNUSED;

    /* To make this simpler this literally just always replies */
    while (true) {
        /* We don't do any measurements here */
        tag = seL4_Recv(INPUT_CAP, &badge, REPLY_CAP);
        seL4_Signal(BASE_OUTPUT_NOTIFICATION_CAP + SIGNAL_MID_HIGH_CHANNEL);
    }
}

DECLARE_SUBVERTED_MICROKIT()
