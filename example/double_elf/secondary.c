/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

static __inline__ uint32_t get_pc(void)  {
    uint32_t pc;
    asm("mov %0, $pc" : "=r"(pc));
    return pc;
}

void init(void)
{
    microkit_dbg_puts("program counter is: ");
    microkit_dbg_put32((uint32_t)(uintptr_t) __builtin_return_address(0));
    microkit_dbg_puts("\n");
    microkit_dbg_puts("if you can read this, it means I was loaded!\n");
    microkit_dbg_puts("checking for this string\n");
    microkit_dbg_puts("third string... now jumping\n");
}

void notified(microkit_channel ch)
{
}