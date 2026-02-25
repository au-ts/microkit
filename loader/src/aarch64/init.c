/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include "../arch.h"
#include "../loader.h"
#include "../uart.h"
#include "el.h"
#include "smc.h"
#include "gic.h"

#include <kernel/gen_config.h>

void el1_mmu_disable(void);
void el2_mmu_disable(void);

void arch_init(void)
{
// FIXME: These were taken from the seL4 kernel, which is GPL-licensed
//        This is not like a thing that we should be doing.
#if defined(GIC_VERSION)
#if GIC_VERSION == 2
    puts("LDR|INFO: Initialising interrupt controller GICv2\n");
    configure_gicv2();
#elif GIC_VERSION == 3
    puts("LDR|INFO: Initialising interrupt controller GICv3\n");
    configure_gicv3();
#else
    #error "unknown GIC version"
#endif
#else
    puts("LDR|INFO: No interrupt controller to initialise\n");
#endif

    /* Disable the MMU, as U-Boot will start in virtual memory on some platforms
     * (https://docs.u-boot.org/en/latest/arch/arm64.html), which means that
     * certain physical memory addresses contain page table information which
     * the loader doesn't know about and would need to be careful not to
     * overwrite.
     *
     * This also means that we would need to worry about caching.
     * TODO: should we do that instead?
     * note the issues where it forces us to flush any shared addresses all the
     * way to cache as we might have mixed non-cached/cached access.
     */
    puts("LDR|INFO: disabling MMU (if it was enabled)\n");
    enum el el = current_el();

    if (el == EL1) {
        el1_mmu_disable();
    } else if (el == EL2) {
        el2_mmu_disable();
    } else {
        puts("LDR|ERROR: unknown EL level for MMU disable\n");
    }

    // TODO: handle non-PSCI platforms better, see https://github.com/seL4/microkit/issues/401.
#if !defined(CONFIG_PLAT_BCM2711)
    uint32_t ret = arm_smc32_call(PSCI_FUNCTION_VERSION, /* unused */ 0, 0, 0);
    /* the return value has no error codes, but if we get it wrong this is what we will get */
    if (ret == PSCI_RETURN_NOT_SUPPORTED) {
        puts("LDR|ERROR: could not determine PSCI version: ");
        puts(psci_return_as_string(ret));
        puts("\n");
    } else {
        uint16_t major = (ret >> 16) & 0xffff;
        uint16_t minor = (ret >>  0) & 0xffff;
        puts("LDR|INFO: PSCI version is ");
        putdecimal(major);
        puts(".");
        putdecimal(minor);
        puts("\n");
    }
#endif
}

typedef void (*sel4_entry)(
    uintptr_t ui_p_reg_start,
    uintptr_t ui_p_reg_end,
    intptr_t pv_offset,
    uintptr_t v_entry,
    uintptr_t dtb_addr_p,
    uintptr_t dtb_size
);

void arch_jump_to_kernel(int logical_cpu)
{
    /* seL4 always expects the current logical CPU number in TPIDR_EL1 */
    asm volatile("msr TPIDR_EL1, %0" :: "r"(logical_cpu));

    // ((sel4_entry)(loader_data->kernel_v_entry))(
    //     (uintptr_t)&loader_data->kernel_bootinfos_and_regions[id].info
    // );
}
