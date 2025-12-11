/*
 * Test
 */

#include <microkit.h>

void init(void)
{
    microkit_dbg_puts("Dynamic PD initialized!\n");
}

void notified(unsigned int ch)
{
    microkit_dbg_puts("Dynamic PD received notification on channel: ");
    microkit_dbg_put32(ch);
    microkit_dbg_puts("\n");
}
