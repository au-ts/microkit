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

void el1_mmu_enable(uint64_t ttbr0_el1, uint64_t ttbr1_el1);
void el2_mmu_enable(uint64_t ttbr0_el2);

struct AArch64ReturnValue {
    uintptr_t ttbr0_el2;
    uintptr_t ttbr0_el1;
    uintptr_t ttbr1_el1;
};

struct Region {
    uint64_t start;
    uint64_t end;
};

const struct Region ram_regions[] = {
    { .start = 0x60000000, .end = 0xc0000000 },
};

const struct Region device_regions[] = {
    { .start = 0x9000000, .end = 0x9000000 + 4096 },
};

uint8_t page_table_bytes[4096][64] ALIGN(4096);
uint8_t regions[16 * 4] ALIGN(16);

extern struct AArch64ReturnValue aarch64_setup_pagetables(
    uint64_t kernel_first_vaddr, uint64_t kernel_first_paddr,
    const void *ram_regions_ptr, uintptr_t ram_regions_len,
    const void *device_regions_ptr, uintptr_t device_regions_len,
    uint8_t page_table_bytes[4096][64],
    uint8_t regions[16 * 4]);

int arch_mmu_enable(int logical_cpu)
{
    struct AArch64ReturnValue pt = aarch64_setup_pagetables(
        0x8060000000, 0x60000000,
        &ram_regions, ARRAY_SIZE(ram_regions),
        &device_regions, ARRAY_SIZE(device_regions),
        page_table_bytes, regions
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
