/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

uintptr_t small_memory_region_vaddr;
uintptr_t small_memory_region_paddr;
uint64_t small_memory_region_size;

static char hexchar(unsigned int v)
{
    return v < 10 ? '0' + v : ('a' - 10) + v;
}

static void puthex64(uint64_t val)
{
    char buffer[16 + 3];
    buffer[0] = '0';
    buffer[1] = 'x';
    buffer[16 + 3 - 1] = 0;
    for (unsigned i = 16 + 1; i > 1; i--) {
        buffer[i] = hexchar(val & 0xf);
        val >>= 4;
    }
    microkit_dbg_puts(buffer);
}

void init(void)
{
    microkit_dbg_puts("hello, world\n");

    microkit_dbg_puts("small region vaddr: ");
    puthex64(small_memory_region_vaddr);
    microkit_dbg_puts("\n");

    microkit_dbg_puts("small region paddr: ");
    puthex64(small_memory_region_paddr);
    microkit_dbg_puts("\n");

    microkit_dbg_puts("small region size: ");
    puthex64(small_memory_region_size);
    microkit_dbg_puts("\n");

    microkit_dbg_puts("writing data to vaddr...");
    char *c = (char *) small_memory_region_vaddr;
    *c = 's';
    microkit_dbg_puts("reading back...");
    if (*c == 's') {
        microkit_dbg_puts("OK\n");
    } else {
        microkit_dbg_puts("FAIL\n");
    }
}

void notified(microkit_channel ch)
{
}
