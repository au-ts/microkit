/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <microkit.h>
#include <stdint.h>
#include <unistd.h>

#define CONTROLLER_CH 0

void init(void)
{
    microkit_dbg_puts("hello, world! I am the first program running in this pd\n");

    for (int i = 0; i < 5; i++)
    {
        microkit_dbg_puts("original vm executing\n");
    }

    microkit_dbg_puts("Now giving control to controller\n");
    (void)microkit_ppcall(CONTROLLER_CH, microkit_msginfo_new(1, 1));
}

void notified(microkit_channel ch)
{
}
