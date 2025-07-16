/*
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

__attribute__((noinline, noclone))
void recurse() {
    microkit_dbg_puts("recurse()\n");
    
    volatile char useless[1024];
    for (int i = 0; i < 1024; i++) {
        useless[i] = (char) 0;
    }

    recurse();
    useless[0] = (char) 0;
}

void
init(void)
{
    microkit_dbg_puts("STACK OVERFLOW PD STARTING\n");
    microkit_dbg_puts("STACK OVERFLOW PD GOING OFF NOW!!!!!!!!\n");
    recurse((char *) 0);
}

void
notified(microkit_channel ch)
{
}
