/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>
#include <sel4/sel4.h>

uintptr_t child_id = 0;
uintptr_t child_channel_id = 0;
// TODO: it seems that the virq_id doesn't do anything rn.
uintptr_t vpmu_cap = BASE_USER_CAPS;
uintptr_t child_tcb_cap = 0;

// Source - https://stackoverflow.com/a/59389473
// Posted by Rida Shamasneh
// Retrieved 2026-08-20, License - CC BY-SA 4.0
void print_int(int num) {
    if (num < 0)
    {
       microkit_dbg_putc('-');
       num = -num;
    }

    if (num > 9) print_int(num/10);

    microkit_dbg_putc('0'+ (num%10));
}


void init(void)
{
    microkit_dbg_puts("parent init\n");
    child_tcb_cap = BASE_TCB_CAP + child_id;
    // bind the vpmu to the child, and initialise it.
    microkit_dbg_puts("Num counters: ");
    microkit_dbg_putc('0' + seL4_ARM_VPMU_VPMUNumCounters(vpmu_cap).num_counters);
    microkit_dbg_puts("\n");
    seL4_TCB_Suspend(child_tcb_cap);

    print_int(seL4_TCB_BindVPMU(child_tcb_cap, vpmu_cap));
    microkit_dbg_puts("\n");
    print_int(seL4_ARM_VPMU_VPMUCounterControl(vpmu_cap, 1));
    microkit_dbg_puts("\n");

    seL4_TCB_Resume(child_tcb_cap);
}

uintptr_t count = 0;
void notified(microkit_channel ch)
{
    seL4_ARM_VPMU_VPMUReadCycleCounter_t res = seL4_ARM_VPMU_VPMUReadCycleCounter(vpmu_cap);
    microkit_dbg_puts("errorval: ");
    print_int(res.error);
    microkit_dbg_puts(" cycle count: ");
    print_int(res.cycle_counter_value);
    microkit_dbg_puts("\n");
    if (count++ == 0) microkit_notify(child_channel_id);
}
seL4_Bool fault(microkit_child child, microkit_msginfo msginfo,
                microkit_msginfo *reply_msginfo) {
    return false;
}

