#include <microkit.h>
#include <stdint.h>

// MMAP_THRESHOLD stolen from AOS project.
#define SIZE_ALIGN (4*sizeof(size_t))
#define MMAP_THRESHOLD (0x1c00*SIZE_ALIGN)

#define 
// malloc and mmap
// has separate address spaces for each process.
// malloc and mmap returns the virtual address of the beginning of the mapped address.

// there shouldn't be a stack so...

void init(void)
{
    // TODO:
}

void notified(microkit_channel ch)
{
    // this may not be required
}

seL4_MessageInfo_t protected(microkit_channel ch, microkit_msginfo msginfo)
{
    // this is called when malloc is called.
    // if there
}

seL4_Bool fault(microkit_child child, microkit_msginfo msginfo, microkit_msginfo *reply_msginfo)
{
    // not required.
}



