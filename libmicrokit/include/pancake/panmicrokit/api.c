/*
 * Copyright 2026, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include <sel4/sel4.h>

void microkit_dbg_putc(int c) {
#if defined(CONFIG_PRINTING)
    seL4_DebugPutChar(c);
#endif
}

void microkit_dbg_puts(const char *s) {
    while (*s) {
        microkit_dbg_putc(*s);
        s++;
    }
}

void microkit_dbg_put8(seL4_Uint8 x) {
    char tmp[4];
    unsigned i = 3;
    tmp[3] = 0;
    do {
        seL4_Uint8 c = x % 10;
        tmp[--i] = '0' + c;
        x /= 10;
    } while (x);
    microkit_dbg_puts(&tmp[i]);
}

void microkit_dbg_put32(seL4_Uint32 x) {
    char tmp[11];
    unsigned i = 10;
    tmp[10] = 0;
    do {
        seL4_Uint8 c = x % 10;
        tmp[--i] = '0' + c;
        x /= 10;
    } while (x);
    microkit_dbg_puts(&tmp[i]);
}


void __assert_fail(const char *assertion, const char *file, int line, const char *function) {
    microkit_dbg_puts("assert failed\n");
    for (;;) {}
}


void microkit_report_debug(long msg) {
    switch (msg) {
        case 1:
            microkit_dbg_puts("microkit_report_debug: running uep_init\n");
            break;
        case 2:
            microkit_dbg_puts("microkit_report_debug: running uep_notified\n");
            break;
        case 3:
            microkit_dbg_puts("microkit_report_debug: running uep_protected\n");
            break;
        case 4:
            microkit_dbg_puts("microkit_report_debug: running uep_fault\n");
            break;
        default:
            microkit_dbg_puts("microkit_report_debug: MISSING STRING\n");
            break;
    }
}

void ffimicrokit_report_debug(unsigned char* c, long clen, unsigned char* a, long alen) {
    long msg = alen;
    microkit_report_debug(msg);
}

void microkit_report_error(long msg, seL4_Error err) {
    switch (msg) {
        case 1:
            microkit_dbg_puts("microkit_pd_stop: error writing TCB registers\n");
            break;
        case 2:
            microkit_dbg_puts("microkit_pd_restart: error writing TCB registers\n");
            break;
        default:
            microkit_dbg_puts("microkit_report_error: unknown error\n");
            break;
    }
    int *x = (int*)(seL4_Word)err;
    *x = 0;
}

void ffimicrokit_report_error(unsigned char* c, long clen, unsigned char* a, long alen) {
    long msg = clen;
    seL4_Error err = (seL4_Error)alen;
    microkit_report_error(msg, err);
}
