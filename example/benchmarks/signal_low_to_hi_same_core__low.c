/*
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

#include "benchmark.h"

uintptr_t shared;

void init(void)
{
    print("hello world\n");

    seL4_Word badge;
    seL4_MessageInfo_t tag = seL4_Recv(INPUT_CAP, &badge, REPLY_CAP);
    print("was told to start.. pretending done\n");
    microkit_notify(BENCHMARK_START_STOP_CH);
}

DECLARE_SUBVERTED_MICROKIT()
