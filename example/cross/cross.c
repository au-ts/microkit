#include <stdint.h>
#include <microkit.h>

void init(void)
{
    microkit_dbg_puts("cross: init()\n");
}

void notified(microkit_channel ch)
{
    microkit_dbg_puts("cross: notified(");
    microkit_dbg_put32(ch);
    microkit_dbg_puts(")\n");
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
