/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#pragma once

#define STACK_SIZE 4096

#define REGION_TYPE_DATA 1
#define REGION_TYPE_ZERO 2

#ifndef __ASSEMBLER__

#include <stdint.h>
#include <stddef.h>

#include "cpus.h"

#define ALIGN(n)  __attribute__((__aligned__(n)))

struct region {
    uintptr_t load_addr; // this should be updated for subsequent regions by loader.rs
    // size of the data to load
    uintptr_t load_size;
    // size of the data to write. this is useful for zeroing out memory.
    uintptr_t write_size;
    uintptr_t offset;
    uintptr_t type;
};

#include "sel4/bootinfo.h"

struct KernelBootInfoAndRegions {
    seL4_KernelBootInfo info;
    uint8_t regions_memory[4096 - sizeof(seL4_KernelBootInfo)];
};

_Static_assert(sizeof(struct KernelBootInfoAndRegions) == 0x1000);

// Changing this structure is precarious, maybe better to wrap in NUM_MULTIKERNELS IFDEF
struct loader_data {
    uintptr_t magic;
    uintptr_t size;
    uintptr_t flags;
    uintptr_t num_kernels;
    uintptr_t num_regions;
    uintptr_t kernel_v_entry;
    struct KernelBootInfoAndRegions kernel_bootinfos_and_regions[];
};

/* Called from assembly */
void relocation_failed(void);
void relocation_log(uint64_t reloc_addr, uint64_t curr_addr);

#if defined(NUM_MULTIKERNELS) && NUM_MULTIKERNELS > 1
extern uint64_t _stack[NUM_MULTIKERNELS][STACK_SIZE] ALIGN(16);
#else
extern uint64_t _stack[NUM_ACTIVE_CPUS][STACK_SIZE / sizeof(uint64_t)];
#endif

void start_kernel(int logical_cpu);

#endif
