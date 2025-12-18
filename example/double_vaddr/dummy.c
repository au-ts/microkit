/* Dummy program to be compiled into an ELF and placed into a memory region
 * This PD is not declared in the system XML; its ELF will be packaged into
 * the dynamic blob and embedded in the `dynamic_elfs_region` memory region.
 */

#include <microkit.h>

void init(void)
{
    microkit_dbg_puts("Dummy PD initialized!\n");
}

void notified(unsigned int ch)
{
    microkit_dbg_puts("Dummy PD got notification on channel: ");
    microkit_dbg_put32(ch);
    microkit_dbg_puts("\n");
}
