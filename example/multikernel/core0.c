/*
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

#define print(str) do { microkit_dbg_puts(microkit_name); microkit_dbg_puts(": "); microkit_dbg_puts(str); } while (0)

void init(void)
{
    print("hello, world (from core 0)\n");

    print("notifying same core on 5\n");
    microkit_notify(5);
}

void notified(microkit_channel ch)
{
    print("notified: ");
    microkit_dbg_put32(ch);

    if (ch == 5) {
        microkit_dbg_puts(" (same core)\n");
    } else if (ch == 0) {
        microkit_dbg_puts(" (cross core)\n");
        print("replying from core 0 to core 1\n");
        microkit_notify(0);
    } else {
        microkit_dbg_puts(" (unknown)\n");
    }
}
