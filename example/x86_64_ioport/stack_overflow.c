/*
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

void recurse(void) {
    char useless[16];
    recurse();
}

void
init(void)
{
    microkit_dbg_puts("STACK OVERFLOW PD STARTING\n");
    microkit_dbg_puts("STACK OVERFLOW PD GOING OFF NOW!!!!!!!!\n");
    recurse();
}

void
notified(microkit_channel ch)
{
}
