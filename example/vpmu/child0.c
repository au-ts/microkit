/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>
#include <sel4/sel4.h>

uintptr_t parent_channel_id = 0;

void init(void)
{
    microkit_dbg_puts("child0 init\n");
    // notify the parent to let it know that it has initialised.
    microkit_notify(parent_channel_id);
}

void notified(microkit_channel ch)
{
    // spinloop volatile.
    volatile int i = 0;
    while (i < 100000) {
        i+=1;
    }
    microkit_dbg_puts("child0 work finished\n");
    microkit_notify(parent_channel_id);
}
