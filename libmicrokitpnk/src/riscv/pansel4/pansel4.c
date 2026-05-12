/*
 * Copyright 2026, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

/* TODO: Currently compatible with ARCH=riscv64 only! */

/* FFI wrappers for sel4/sel4.h on RISC-V */
/*
 * Naming convention used here:
 *   The Pancake function is `panseL4_<name>(..)`, taking the same args as
 *   as the C function `seL4_<name>(..)` in `sel4.h`.
 *   Under the hood the Pancake function calls the FFI function
 *   `@outer_<name>(c,clen,a,alen)`.
 *   The Pancake runtime calls the C symbol `ffiouter_<name>(c,clen,a,alen)`.
 *   The C calls `seL4_<name>(..)`.
 */

#define __thread
#include <sel4/sel4.h>
#include <stdint.h>
#include <stddef.h>

_Static_assert(sizeof(uint64_t) == 8, "panseL4: ARCH error, expected uint64_t to be 8 bytes");
_Static_assert(sizeof(seL4_Word) == 8, "panseL4: ARCH error, expected seL4_Word to be 8 bytes");
_Static_assert(sizeof(seL4_CPtr) == sizeof(seL4_Word), "panseL4: ARCH error, expected seL4_CPtr to be word-sized");
_Static_assert(sizeof(seL4_MessageInfo_t) == sizeof(seL4_Word), "panseL4: ARCH error, expected seL4_MessageInfo_t to be word-sized");

static inline uint64_t* as_u64_buffer(unsigned char* a) {
    unsigned char* a_bytes = a;
    void* a_void = (void*)a_bytes;
    uint64_t* buf = (uint64_t*)a_void;
    return buf;
}

static inline seL4_MessageInfo_t msginfo_from_u64(uint64_t w) {
    seL4_Word ww = (seL4_Word)w;
    seL4_MessageInfo_t t;
    t.words[0] = ww;
    return t;
}

static inline uint64_t u64_from_msginfo(seL4_MessageInfo_t t) {
    seL4_Word w = t.words[0];
    return (uint64_t)w;
}

void ffiouter_seL4_MessageInfo_new(unsigned char* c, long clen, unsigned char* a, long alen) {
    uint64_t* buf = as_u64_buffer(a);
    uint64_t label_u64 = buf[0];
    uint64_t caps_u64 = buf[1];
    uint64_t extra_u64 = buf[2];
    uint64_t len_u64 = buf[3];
    seL4_Word label = (seL4_Word)label_u64;
    seL4_Word capsUnwrapped = (seL4_Word)caps_u64;
    seL4_Word extraCaps = (seL4_Word)extra_u64;
    seL4_Word length = (seL4_Word)len_u64;
    seL4_MessageInfo_t r = seL4_MessageInfo_new(label, capsUnwrapped, extraCaps, length);
    uint64_t r_u64 = u64_from_msginfo(r);
    buf[0] = r_u64;
}

// TODO: We need this specialized version of ffiouter_seL4_MessageInfo_new
// because IRQAckIRQ is fixed only at build time. Can we get rid of it?
void ffiouter_seL4_MessageInfo_IRQAckIRQ(unsigned char* c, long clen, unsigned char* a, long alen) {
    seL4_MessageInfo_t r = seL4_MessageInfo_new(IRQAckIRQ, 0, 0, 0);
    uint64_t* buf = as_u64_buffer(a);
    uint64_t r_u64 = u64_from_msginfo(r);
    buf[0] = r_u64;
}


void ffiouter_seL4_MessageInfo_get_label(unsigned char* c, long clen, unsigned char* a, long alen) {
    seL4_MessageInfo_t msg = msginfo_from_u64((uint64_t)alen);
    seL4_Word label = seL4_MessageInfo_get_label(msg);
    uint64_t* buf = as_u64_buffer(a);
    buf[0] = (uint64_t)label;
}

void ffiouter_seL4_MessageInfo_get_length(unsigned char* c, long clen, unsigned char* a, long alen) {
    seL4_MessageInfo_t msg = msginfo_from_u64((uint64_t)alen);
    seL4_Word length = seL4_MessageInfo_get_length(msg);
    uint64_t* buf = as_u64_buffer(a);
    buf[0] = (uint64_t)length;
}

void ffiouter_seL4_SetMR(unsigned char* c, long clen, unsigned char* a, long alen) {
    seL4_Uint8 mr = (seL4_Uint8)clen;
    seL4_Word value = (seL4_Word)alen;
    seL4_SetMR(mr, value);
}

void ffiouter_seL4_GetMR(unsigned char* c, long clen, unsigned char* a, long alen) {
    seL4_Uint8 mr = (seL4_Uint8)alen;
    seL4_Word value = seL4_GetMR(mr);
    uint64_t* buf = as_u64_buffer(a);
    buf[0] = (uint64_t)value;
}

void ffiouter_seL4_Signal(unsigned char* c, long clen, unsigned char* a, long alen) {
    seL4_Word dest_word = (seL4_Word)alen;
    seL4_CPtr dest = (seL4_CPtr)dest_word;
    seL4_Signal(dest);
}

void ffiouter_seL4_IRQHandler_Ack(unsigned char* c, long clen, unsigned char* a, long alen) {
    seL4_Word service_word = (seL4_Word)alen;
    seL4_CPtr irqhandler = (seL4_CPtr)service_word;
    seL4_Error err = seL4_IRQHandler_Ack(irqhandler);
    uint64_t* buf = as_u64_buffer(a);
    buf[0] = (uint64_t)err;
}

void ffiouter_seL4_Recv(unsigned char* c, long clen, unsigned char* a, long alen) {
    seL4_Word src_word = (seL4_Word)clen;
    seL4_CPtr src = (seL4_CPtr)src_word;
    seL4_Word reply_word = (seL4_Word)alen;
    seL4_CPtr reply = (seL4_CPtr)reply_word;
    uint64_t* buf = as_u64_buffer(a);
    seL4_Word* badge = (seL4_Word*)(void*)&buf[1];
    seL4_MessageInfo_t r = seL4_Recv(src, badge, reply);
    buf[0] = u64_from_msginfo(r);
}

void ffiouter_seL4_ReplyRecv(unsigned char* c, long clen, unsigned char* a, long alen) {
    uint64_t* buf = as_u64_buffer(a);
    seL4_Word src_word = (seL4_Word)clen;
    seL4_CPtr src = (seL4_CPtr)src_word;
    seL4_MessageInfo_t msg = msginfo_from_u64((uint64_t)alen);
    uint64_t reply_u64 = buf[0];
    seL4_Word reply_word = (seL4_Word)reply_u64;
    seL4_CPtr reply = (seL4_CPtr)reply_word;
    seL4_Word* badge = (seL4_Word*)(void*)&buf[1];
    seL4_MessageInfo_t r = seL4_ReplyRecv(src, msg, badge, reply);
    buf[0] = u64_from_msginfo(r);
}

void ffiouter_seL4_NBSendRecv(unsigned char* c, long clen, unsigned char* a, long alen) {
    uint64_t* buf = as_u64_buffer(a);
    seL4_Word dest_word = (seL4_Word)clen;
    seL4_CPtr dest = (seL4_CPtr)dest_word;
    seL4_MessageInfo_t msg = msginfo_from_u64((uint64_t)alen);
    uint64_t src_u64 = buf[0];
    seL4_Word src_word = (seL4_Word)src_u64;
    seL4_CPtr src = (seL4_CPtr)src_word;
    uint64_t reply_u64 = buf[1];
    seL4_Word reply_word = (seL4_Word)reply_u64;
    seL4_CPtr reply = (seL4_CPtr)reply_word;
    seL4_Word* badge = (seL4_Word*)(void*)&buf[1];
    seL4_MessageInfo_t r = seL4_NBSendRecv(dest, msg, src, badge, reply);
    buf[0] = u64_from_msginfo(r);
}

void ffiouter_seL4_Call(unsigned char* c, long clen, unsigned char* a, long alen) {
    seL4_Word dest_word = (seL4_Word)clen;
    seL4_CPtr dest = (seL4_CPtr)dest_word;
    seL4_MessageInfo_t msg = msginfo_from_u64(alen);
    seL4_MessageInfo_t r = seL4_Call(dest, msg);
    uint64_t* buf = as_u64_buffer(a);
    buf[0] = u64_from_msginfo(r);
}

// TODO: this has a partial/specialized implementation below, under the name
// ffiouter_seL4_TCB_WriteRegisters_spec, which is sufficient for libmicrokit.
// void ffiouter_seL4_TCB_WriteRegisters(unsigned char* c, long clen, unsigned char* a, long alen)

void ffiouter_seL4_TCB_WriteRegisters_spec(unsigned char* c, long clen, unsigned char* a, long alen) {
    seL4_TCB tcb_cap = (seL4_TCB)clen;
    seL4_UserContext ctxt = {0};
    ctxt.pc = (seL4_Word)alen;
    seL4_Error err = seL4_TCB_WriteRegisters(tcb_cap, seL4_True, 0, 1, &ctxt);
    uint64_t* buf = as_u64_buffer(a);
    buf[0] = (uint64_t)err;
    return;
}

void ffiouter_seL4_TCB_Suspend(unsigned char* c, long clen, unsigned char* a, long alen) {
    seL4_Word tcb_word = (seL4_Word)alen;
    seL4_CPtr tcb = (seL4_CPtr)tcb_word;
    seL4_Error err = seL4_TCB_Suspend(tcb);
    uint64_t* buf = as_u64_buffer(a);
    buf[0] = (uint64_t)err;
}
