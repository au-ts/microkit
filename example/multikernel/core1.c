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
    microkit_dbg_puts(" says: hello, world (from core 1)\n");

    // microkit_dbg_puts("signalling from core 1 to core 0\n");
    // seL4_Signal(0xf01);
}

void notified(microkit_channel ch)
{

}
