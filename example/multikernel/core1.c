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
    print("hello, world (from core 1)\n");
    print("signalling from core 1 to core 0\n");
    microkit_notify(0);
}

int notified_count = 5;

void notified(microkit_channel ch)
{
    print("notified: ");
    microkit_dbg_put32(ch);

    if (ch == 0) {
        microkit_dbg_puts(" (cross core)\n");

        if (notified_count > 0) {
            print("replying from core 1 to core 0\n");
            microkit_notify(0);
            notified_count--;
        } else {
            print("stopping after 5 notifications\n");
        }
    } else {
        microkit_dbg_puts(" (unknown)\n");
    }
}
