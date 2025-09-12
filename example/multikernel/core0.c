/*
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

void init(void)
{
    microkit_dbg_puts(microkit_name);
    microkit_dbg_puts(" says: hello, world (from core 0)\n");

    microkit_dbg_puts("notifying intra-core\n");
    microkit_notify(5);
}

void notified(microkit_channel ch)
{
    microkit_dbg_puts(microkit_name);
    microkit_dbg_puts(" notified: ");
    microkit_dbg_put32(ch);
    microkit_dbg_puts("\n");
}
