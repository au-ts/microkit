/*
 * Copyright 2026, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

void init(void)
{
    microkit_dbg_puts("pong: initialized\n");
}

void notified(microkit_channel ch)
{
    microkit_dbg_puts("pong: got pinged on channel ");
    microkit_dbg_put32(ch);
    microkit_dbg_puts("\n");
    microkit_notify(2);
}
