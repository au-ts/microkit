/*
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

#define print(str) do { microkit_dbg_puts(microkit_name); microkit_dbg_puts(": "); microkit_dbg_puts(str); } while (0)

uintptr_t shared_v;
uintptr_t shared_p;

static void print_and_modify_shared(void) {
    volatile char *shared = (volatile void *)shared_v;

    print("shared value: ");
    microkit_dbg_put32(*shared);
    microkit_dbg_puts("\n");

    *shared = 128;

    print("new shared value: ");
    microkit_dbg_put32(*shared);
    microkit_dbg_puts("\n");
}

void init(void)
{
    print("hello, world (from core 1)\n");

    print("shared_v: ");
    microkit_dbg_put32(shared_v);
    microkit_dbg_puts("\n");
    print("shared_p: ");
    microkit_dbg_put32(shared_p);
    microkit_dbg_puts("\n");

    print_and_modify_shared();

    microkit_notify(0);
}

void notified(microkit_channel ch)
{
    print("notified: ");
    microkit_dbg_put32(ch);

    if (ch == 0) {
        microkit_dbg_puts(" (cross core)\n");
    } else {
        microkit_dbg_puts(" (unknown)\n");
    }
}
