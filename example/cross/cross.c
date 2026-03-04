#include <stdint.h>
#include "panmicrokit.h"

void init(void)
{
    microkit_dbg_puts("cross: init()\n");
}

void ffic_init(unsigned char* c, long clen, unsigned char* a, long alen) {
    init();
}

void notified(microkit_channel ch)
{
    microkit_dbg_puts("cross: notified(");
    microkit_dbg_put32(ch);
    microkit_dbg_puts(")\n");
}

void ffic_notified(unsigned char* c, long clen, unsigned char* a, long alen) {
    microkit_channel ch = (microkit_channel)alen;
    notified(ch);
}

microkit_msginfo protected(microkit_channel ch, microkit_msginfo msginfo)
{
    microkit_dbg_puts("cross: protected(");
    microkit_dbg_put32(ch);
    microkit_dbg_puts(",");
    microkit_dbg_put32(msginfo.words[0]);
    microkit_dbg_puts(")\n");
    return microkit_msginfo_new(0,0);
}

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
