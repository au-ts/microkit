/*
 * Copyright 2026, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

/* The Pancake-compatible part of the Microkit interface. This is separated
 * from microkit.h so that it can be included when running a C protection
 * domain linked against Pancake libmicrokit (panmicrokit) instead of the
 * default C implementation of libmicrokit.
 *
 * Make sure the constants here coincide with the ones in `microkit.h`!
 */

#pragma once

#include <stdint.h>
#define __thread
#include <sel4/sel4.h>

typedef unsigned int microkit_channel;
typedef unsigned int microkit_child;
typedef unsigned int microkit_ioport;
typedef seL4_MessageInfo_t microkit_msginfo;

#define MONITOR_EP 5
/* Only valid in the 'benchmark' configuration */
#define TCB_CAP 6
/* Only valid when the PD has been configured to make SMC calls */
#define ARM_SMC_CAP 7
#define BASE_OUTPUT_NOTIFICATION_CAP 10
#define BASE_ENDPOINT_CAP 74
#define BASE_IRQ_CAP 138
#define BASE_TCB_CAP 202
#define BASE_VM_TCB_CAP 266
#define BASE_VCPU_CAP 330
#define BASE_IOPORT_CAP 394

#define MICROKIT_MAX_CHANNELS 62
#define MICROKIT_MAX_CHANNEL_ID (MICROKIT_MAX_CHANNELS - 1)
#define MICROKIT_MAX_IOPORT_ID MICROKIT_MAX_CHANNELS
#define MICROKIT_PD_NAME_LENGTH 64

extern char microkit_name[MICROKIT_PD_NAME_LENGTH];

/* Symbols for error checking libmicrokit API calls. Patched by the Microkit tool
 * to set bits corresponding to valid channels for this PD. */
extern seL4_Word microkit_irqs;
extern seL4_Word microkit_notifications;
extern seL4_Word microkit_pps;
extern seL4_Word microkit_ioports;

extern void microkit_notify(microkit_channel ch);
extern void microkit_irq_ack(microkit_channel ch);
extern seL4_Error microkit_pd_stop(microkit_child pd);
extern seL4_Error microkit_pd_restart(microkit_child pd, seL4_Word entry_point);
extern microkit_msginfo microkit_ppcall(microkit_channel ch, microkit_msginfo msginfo);
extern microkit_msginfo microkit_msginfo_new(seL4_Word label, seL4_Uint16 count);
extern void microkit_deferred_notify(microkit_channel ch);
extern void microkit_deferred_irq_ack(microkit_channel ch);

/* User-provided entry points and FFI required for Pancake-C interop */
void init(void);
void ffic_init(unsigned char* c, long clen, unsigned char* a, long alen) {
    init();
}

void notified(microkit_channel ch);
void ffic_notified(unsigned char* c, long clen, unsigned char* a, long alen) {
    microkit_channel ch = (microkit_channel)alen;
    notified(ch);
}

microkit_msginfo protected(microkit_channel ch, microkit_msginfo msginfo);
void ffic_protected(unsigned char* c, long clen, unsigned char* a, long alen) {
    unsigned char* a_bytes = a;
    void* a_void = (void*)a_bytes;
    uint64_t* buf = (uint64_t*)a_void;
    microkit_channel ch = (microkit_channel)clen;
    microkit_msginfo msginfo;
    msginfo.words[0] = (seL4_Word)alen;
    microkit_msginfo reply_tag = protected(ch, msginfo);
    buf[0] = (uint64_t)reply_tag.words[0];
    return;
}

// seL4_Bool fault(microkit_child child, microkit_msginfo msginfo, microkit_msginfo *reply_msginfo)
// TODO: fault currently unsupported in C-over-Pancake

/*
 * Output a single character on the debug console.
 */
void microkit_dbg_putc(int c);

/*
 * Output a NUL terminated string to the debug console.
 */
void microkit_dbg_puts(const char *s);

/*
 * Output the decimal representation of an 8-bit integer to the debug console.
 */
void microkit_dbg_put8(seL4_Uint8 x);

/*
 * Output the decimal representation of an 32-bit integer to the debug console.
 */
void microkit_dbg_put32(seL4_Uint32 x);

static inline void microkit_internal_crash(seL4_Error err)
{
    int *x = (int *)(seL4_Word) err;
    *x = 0;
}

static inline seL4_Word microkit_msginfo_get_label(microkit_msginfo msginfo)
{
    return seL4_MessageInfo_get_label(msginfo);
}

static inline seL4_Word microkit_msginfo_get_count(microkit_msginfo msginfo)
{
    return seL4_MessageInfo_get_length(msginfo);
}

static inline void microkit_mr_set(seL4_Uint8 mr, seL4_Word value)
{
    seL4_SetMR(mr, value);
}

static inline seL4_Word microkit_mr_get(seL4_Uint8 mr)
{
    return seL4_GetMR(mr);
}

seL4_Bool fault(microkit_child child, microkit_msginfo msginfo, microkit_msginfo *reply_msginfo)
{
    microkit_dbg_puts(microkit_name);
    microkit_dbg_puts(" is missing the 'fault' entry point, unsupported for C PDs on Pancake Microkit\n");
    microkit_internal_crash(0);
    return seL4_False;
}
