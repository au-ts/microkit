/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include <stdint.h>

#include "el.h"
#include "../arch.h"
#include "../cutil.h"
#include "../uart.h"

void el1_mmu_enable(uint64_t aarch64_pt_ttbr0_el1, uint64_t aarch64_pt_ttbr1_el1);
void el2_mmu_enable(uint64_t aarch64_pt_ttbr0_el2);

/* Pointers to the top-level paging structures */
uint64_t aarch64_pt_ttbr0_el1;
uint64_t aarch64_pt_ttbr1_el1;
uint64_t aarch64_pt_ttbr0_el2;

struct ret {
    uint64_t a;
    uint64_t b;
    uint64_t c;
};

extern struct ret aarch64_setup_pagetables(uint64_t kernel_first_vaddr, uint64_t kernel_first_paddr, uint64_t page_tables_paddr_start);

int arch_mmu_enable(int logical_cpu)
{
    puts("setup1\n");
    struct ret x = aarch64_setup_pagetables(0, 0, 0);
    aarch64_pt_ttbr0_el1 = x.a;
    aarch64_pt_ttbr1_el1 = x.b;
    aarch64_pt_ttbr0_el2 = x.c;
    puts("setup\n");

    int r;
    enum el el;
    r = ensure_correct_el(logical_cpu);
    if (r != 0) {
        return r;
    }

    LDR_PRINT("INFO", logical_cpu, "enabling MMU\n");
    el = current_el();
    if (el == EL1) {
        el1_mmu_enable(aarch64_pt_ttbr0_el1, aarch64_pt_ttbr1_el1);
    } else if (el == EL2) {
        el2_mmu_enable(aarch64_pt_ttbr0_el2);
    } else {
        LDR_PRINT("ERROR", logical_cpu, "unknown EL for MMU enable\n");
    }

    return 0;
}
