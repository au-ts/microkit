/*
 * Copyright 2026, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

/* The adjutant is the Pancake runtime and has the main entry point. */

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define __thread
#include <sel4/sel4.h>

/* a minimal Pancake runtime */
static char cml_memory[1024 * 12];
extern void* cml_heap;
extern void* cml_stack;
extern void* cml_stackend;
extern void cml_main();
void cml_exit(int arg) { }
void cml_err(int arg) { cml_exit(arg); }
void cml_clear() {}
static void init_pancake_mem() {
    unsigned long heap_sz = 1024 * 9;
    unsigned long stack_sz = 1024 * 3;
    cml_heap = cml_memory;
    cml_stack = (char*)cml_heap + heap_sz;
    cml_stackend = (char*)cml_stack + stack_sz;
}

/* symbols expected by the Microkit tool */
bool microkit_passive;
char microkit_name[64];
seL4_Word microkit_irqs;
seL4_Word microkit_notifications;
seL4_Word microkit_pps;
seL4_Word microkit_ioports;
extern const void (*const __init_array_start[])();
extern const void (*const __init_array_end[])();

/* symbols expected by libseL4 */
extern seL4_IPCBuffer __sel4_ipc_buffer_obj;
seL4_IPCBuffer *__sel4_ipc_buffer = &__sel4_ipc_buffer_obj;

static void run_init_funcs(void)
{
    size_t n = (size_t)(__init_array_end - __init_array_start);
    for (size_t i = 0; i < n; i++) {
        __init_array_start[i]();
    }
}

void ffimicrokit_get_constant(unsigned char* c, long clen, unsigned char* a, long alen) {
    unsigned char* a_bytes = a;
    void* a_void = (void*)a_bytes;
    uint64_t* buf = (uint64_t*)a_void;
    switch (alen) {
        case 0:
            buf[0] = microkit_irqs;
            break;
        case 1:
            buf[0] = microkit_notifications;
            break;
        case 2:
            buf[0] = microkit_pps;
            break;
        case 3:
            buf[0] = microkit_ioports;
            break;
        default:
            break;
    }
}

void main()
{
    run_init_funcs();
    init_pancake_mem();
    cml_main();
    for (;;) {}
}
