/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

void init(void)
{
    microkit_dbg_puts("hello, world\n");
    microkit_dbg_put32((uint32_t)(uintptr_t) __builtin_return_address(0));
    microkit_dbg_puts("\n");

    void (*secondary_addr)(void ) = (void (*)(void)) 0x400000;
    secondary_addr();
}

void notified(microkit_channel ch)
{
}
