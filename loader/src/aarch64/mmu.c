/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include <stdint.h>
#include <stdbool.h>

#include "el.h"
#include "../arch.h"
#include "../cutil.h"
#include "../uart.h"

void el1_mmu_enable(uint64_t ttbr0_el1, uint64_t ttbr1_el1);
void el2_mmu_enable(uint64_t ttbr0_el2);

struct AArch64ReturnValue {
    uintptr_t ttbr0_el2;
    uintptr_t ttbr0_el1;
    uintptr_t ttbr1_el1;
};

union RegionArchAttrs {
    bool is_ram;
    uint64_t raw;
};

struct Region {
    uintptr_t start;
    uintptr_t top;
    union RegionArchAttrs arch_attrs;
};

struct Region regions[] = {
    { .start = 0x60000000, .top = 0xc0000000 - 1, .arch_attrs.is_ram = true },
    { .start = 0x9000000, .top = 0x9000000 + 0xfff, .arch_attrs.is_ram = false },
};

#define PAGE_TABLE_SIZE 4096
#define MAX_NUM_PAGE_TABLES 64

uint8_t page_table_bytes[PAGE_TABLE_SIZE][MAX_NUM_PAGE_TABLES] ALIGN(4096);

extern struct AArch64ReturnValue aarch64_setup_pagetables(
    uint64_t kernel_first_vaddr, uint64_t kernel_first_paddr,
    void *regions_ptr, uintptr_t regions_len,
    uint8_t page_table_bytes[4096][64]);

int arch_mmu_enable(int logical_cpu)
{
    struct AArch64ReturnValue pt = aarch64_setup_pagetables(
        0x8060000000, 0x60000000,
        &regions, ARRAY_SIZE(regions),
        page_table_bytes
    );

    int r;
    enum el el;
    r = ensure_correct_el(logical_cpu);
    if (r != 0) {
        return r;
    }

    LDR_PRINT("INFO", logical_cpu, "enabling MMU\n");
    el = current_el();
    if (el == EL1) {
        el1_mmu_enable(pt.ttbr0_el1, pt.ttbr1_el1);
    } else if (el == EL2) {
        el2_mmu_enable(pt.ttbr0_el2);
    } else {
        LDR_PRINT("ERROR", logical_cpu, "unknown EL for MMU enable\n");
    }

    return 0;
}
