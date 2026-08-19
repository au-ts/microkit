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
    struct seL4_ARM_VPMU_VPMUNumCounters res = seL4_ARM_VPMU_VPMUNumCounters(BASE_IOPORT_CAP + 64);
    if (res.error) {
        microkit_dbg_puts("error: ");
        microkit_dbg_putc('0' + res.error);
        microkit_dbg_puts("\n");
        if (res.error > 9) microkit_dbg_puts("error is larger than 9\n");
    }
    microkit_dbg_puts("Num pmu counters: ");
    microkit_dbg_putc('0' + (char) res.num_counters);
    microkit_dbg_puts("\n");
}

void notified(microkit_channel ch)
{
}
