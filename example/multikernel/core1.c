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

        // NOTE: No need to call microkit_irq_ack(), as the
        // interruptMask() call that seL4 does does nothing for SGI it seems
        // "for SGIs, the behavior of this bit is IMPLEMENTATION DEFINED."
        // for the GICD_ICENABLER bit.
        // ... slightly concerning that this is how it works though
        // see also: https://github.com/seL4/seL4/issues/1185
        //
        //
        // GIC-400 (GICv2) defines it:
        // "The reset value for the register that contains the SGI and PPI interrupts is 0x0000FFFF because SGIs are always enabled."
        // Cortex A-15 (GICv2) defines it:
        // "The reset value for the register that contains the SGI and PPI interrupts is 0x0000FFFF because SGIs are always enabled."
        // Cortex A-9 (GICv2) defines it:
        // "In the Cortex-A9 MPCore, SGIs are always enabled. The corresponding bits in the ICDISERn are read as one, write ignored."
        // Cortex A-7 (GICv2) defines it:
        // "The reset value for the register that contains the SGI and PPI interrupts is 0x0000FFFF because SGIs are always enabled."
        // GIC-500 (GICv3) defines it:
        // "The reset value for the register that contains the SGI and PPI interrupts is 0x0000FFFF because SGIs are always enabled."

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
