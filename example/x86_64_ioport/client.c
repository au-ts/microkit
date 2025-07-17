/*
 * Copyright 2025, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

uintptr_t small_memory_region_vaddr;
uintptr_t small_memory_region_paddr;
uint64_t small_memory_region_size;

uintptr_t large_memory_region_vaddr;
uintptr_t large_memory_region_paddr;
uint64_t large_memory_region_size;

#define SERVER_CH 11

static inline void serial_putc(char ch)
{
    microkit_dbg_putc(ch);
}

static inline void serial_puts(const char *s)
{
    while (*s) {
        if (*s == '\n')
            serial_putc('\r');
        serial_putc(*s++);
    }
}

void
init(void)
{
    serial_puts("CLIENT: booting up, checking shared memory region to server\n");
    char *c = (char *) (small_memory_region_vaddr + 0xfff);
    if (*c == 's') {
        serial_puts("CLIENT: small region OK!\n");
    } else {
        serial_puts("CLIENT: small region NOT EQUAL!\n");
    }
    c = (char *) (large_memory_region_vaddr + 0x1fffff);
    if (*c == 'e') {
        serial_puts("CLIENT: large region OK!\n");
    } else {
        serial_puts("CLIENT: large region NOT EQUAL!\n");
    }

    serial_puts("CLIENT: notifying server\n");
    microkit_notify(SERVER_CH);

    serial_puts("CLIENT: PPC'ing server\n");
    seL4_Word label = 42;
    microkit_msginfo msginfo = microkit_msginfo_new(label, 0);
    msginfo = microkit_ppcall(SERVER_CH, msginfo);

    serial_puts("CLIENT: PPC return\n");

    serial_puts("CLIENT: CRASHING NOW!!!!!\n");
    uint32_t *crash_addr = (uint32_t *) 0xdeadbeef;
    uint32_t crash = *crash_addr;
    (void) crash;
}

void
notified(microkit_channel ch)
{
}
