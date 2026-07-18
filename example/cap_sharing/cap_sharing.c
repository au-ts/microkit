/*
 * Copyright 2026, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <stdbool.h>
#include <microkit.h>

#define CH_SECONDARY ((microkit_channel)0)

// As per cap_sharing.system
#define CAP_SECONDARY_SC  (microkit_cspace_root_slot_to_cptr(1))
#define CAP_SECONDARY_TCB (microkit_cspace_root_slot_to_cptr(2))
#define CAP_MY_SC         (microkit_cspace_root_slot_to_cptr(3))
#define CAP_MY_TCB        (microkit_cspace_root_slot_to_cptr(4))
#define CAP_MY_VSPACE     (microkit_cspace_root_slot_to_cptr(5))
#define CAP_MR            (microkit_cspace_root_slot_to_cptr(6))
#define CAP_IOSPACE       (microkit_cspace_root_slot_to_cptr(7))
#define MR_SIZE           0xA000
#define MR_PAGE_SIZE      0x1000

#define IOVA              0x100000
seL4_Word dma_buffer_vaddr;
static void halt(void)
{
    seL4_Error error = seL4_TCB_Suspend(CAP_MY_TCB);
    if (error != seL4_NoError) {
        microkit_dbg_puts("|primary  | error suspending TCB\n");
    }

    microkit_dbg_puts("|primary  | error: should not reach this point! we should have suspended ourself!\n");
    while (1) { }
}

static void put_hex64(uint64_t value)
{
    char buf[19] = "0x0000000000000000";
    for (seL4_Word i = 0; i < 16; i++) {
        unsigned int nibble = (value >> ((15 - i) * 4)) & 0xf;
        buf[2 + i] = nibble < 10 ? '0' + nibble : 'a' + (nibble - 10);
    }
    microkit_dbg_puts(buf);
}

void init(void)
{
    seL4_Error err;

    microkit_dbg_puts("|primary  | hello, world\n");

    /* Notify the secondary. This will print output from secondary as it is
       higher priority. */
    microkit_dbg_puts("|primary  | notifying secondary\n");
    microkit_notify(CH_SECONDARY);

    microkit_dbg_puts("|primary  | suspending secondary\n");
    err = seL4_TCB_Suspend(CAP_SECONDARY_TCB);
    if (err != seL4_NoError) {
        microkit_dbg_puts("|primary  | error suspending TCB\n");
        halt();
    }

    /* Notify the secondary. It is suspended so it will not print. */
    microkit_dbg_puts("|primary  | notifying secondary (it should not print)\n");
    microkit_notify(CH_SECONDARY);

    microkit_dbg_puts("|primary  | resuming secondary (it should then print)\n");
    err = seL4_TCB_Resume(CAP_SECONDARY_TCB);
    if (err != seL4_NoError) {
        microkit_dbg_puts("|primary  | error resuming TCB\n");
        halt();
    }

    for (seL4_Word i = 0; i < MR_SIZE / MR_PAGE_SIZE; i++) {
        seL4_Word paddr;
        err = microkit_page_get_address(CAP_MR | i, &paddr);
        if (err != seL4_NoError) {
            microkit_dbg_puts("|primary  | error invoking frame capability to retrieve physical address\n");
            halt();
        }

        microkit_dbg_puts("|primary  | frame ");
        put_hex64(i);
        microkit_dbg_puts(" has physical address ");
        put_hex64(paddr);
        microkit_dbg_puts("\n");

        err = microkit_page_map(CAP_MR | i, CAP_MY_VSPACE, dma_buffer_vaddr + i * MR_PAGE_SIZE, seL4_CapRights_new(false,false,true,true));
        if (err != seL4_NoError) {
            microkit_dbg_puts("|primary  | error mapping in a frame capability\n");
            halt();
        }
        volatile char* buf = (volatile char*)dma_buffer_vaddr;
        buf[i * MR_PAGE_SIZE] = 1;

        err = microkit_page_unmap(CAP_MR | i);
        if (err != seL4_NoError) {
            microkit_dbg_puts("|primary  | error unmapping a frame capability\n");
            halt();
        }

        err = microkit_io_page_map(CAP_MR | i, CAP_IOSPACE, seL4_CapRights_new(false,false,true,true), IOVA + i * MR_PAGE_SIZE);
        if (err != seL4_NoError) {
            microkit_dbg_puts("|primary  | error mapping a frame into the io address space\n");
            halt();
        }

        err = microkit_page_unmap(CAP_MR | i);
        if (err != seL4_NoError) {
            microkit_dbg_puts("|primary  | error unmapping a frame capability from the io adddress space\n");
            halt();
        }
    }

    microkit_dbg_puts("|primary  | halting (success)...\n");
    halt();
}

void notified(microkit_channel ch)
{
}
