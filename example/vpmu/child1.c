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
    microkit_dbg_puts("child1 init\n");
    microkit_notify(parent_channel_id);
}

void notified(microkit_channel ch)
{
    // no work.
    microkit_dbg_puts("child1 work finished\n");
    microkit_notify(parent_channel_id);
}
