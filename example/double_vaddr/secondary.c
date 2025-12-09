/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>
#include <stdbool.h>

static __inline__ uint32_t get_pc(void)
{
    uint32_t pc;
    asm("mov %0, $pc" : "=r"(pc));
    return pc;
}

void init(void)
{
    microkit_dbg_puts("hello!! I have been loaded");

    microkit_dbg_puts("secondary vm executing\n");

    microkit_dbg_puts("sched dump secondary\n");
    seL4_DebugDumpScheduler();
}

void notified(microkit_channel ch)
{
}