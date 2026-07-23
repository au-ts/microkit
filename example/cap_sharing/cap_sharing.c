/*
 * Copyright 2026, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

#define CH_SECONDARY ((microkit_channel)0)

// As per cap_sharing.system
#define CAP_SECONDARY_SC     (microkit_cspace_root_slot_to_cptr(1))
#define CAP_SECONDARY_TCB    (microkit_cspace_root_slot_to_cptr(2))
#define CAP_MY_SC            (microkit_cspace_root_slot_to_cptr(3))
#define CAP_MY_TCB           (microkit_cspace_root_slot_to_cptr(4))
#define CAP_MY_VSPACE        (microkit_cspace_root_slot_to_cptr(5))
#define CAP_MR               (microkit_cspace_root_slot_to_cptr(6))
#define CAP_IOSPACE          (microkit_cspace_root_slot_to_cptr(7))
#define CAP_SECONDARY_STACK  (microkit_cspace_root_slot_to_cptr(9))
#define CAP_SECONDARY_IPCBUF (microkit_cspace_root_slot_to_cptr(10))
#define CAP_SECONDARY_ELF    (microkit_cspace_root_slot_to_cptr(11))

#define SLOT_SECONDARY_STACK  9
#define SLOT_SECONDARY_IPCBUF 10
#define SLOT_SECONDARY_ELF    11

#define MR_SIZE              0xa000
#define MR_PAGE_SIZE         0x1000
#define STACK_SIZE           0x2000

#define DMA_BUFFER_VADDR     0x10001000
#define IOVA                 0x100000
#define RUNTIME_IOVA         (IOVA + MR_SIZE)

#if defined(CONFIG_ARCH_X86_64)
#define MICROKIT_EXAMPLE_DEFAULT_VM_ATTRIBUTES seL4_X86_Default_VMAttributes
#elif defined(CONFIG_ARCH_AARCH64)
#define MICROKIT_EXAMPLE_DEFAULT_VM_ATTRIBUTES seL4_ARM_Default_VMAttributes
#elif defined(CONFIG_ARCH_RISCV)
#define MICROKIT_EXAMPLE_DEFAULT_VM_ATTRIBUTES seL4_RISCV_Default_VMAttributes
#else
#error "Unsupported architecture"
#endif

static void halt(void)
{
    seL4_Error error = seL4_TCB_Suspend(CAP_MY_TCB);
    if (error != seL4_NoError) {
        microkit_dbg_puts("|primary  | error suspending TCB\n");
    }

    microkit_dbg_puts("|primary  | error: should not reach this point! we should have suspended ourself!\n");
    while (1) { }
}

static void put_hex64(seL4_Word value)
{
    static const char hex[] = "0123456789abcdef";

    microkit_dbg_puts("0x");
    for (int i = 15; i >= 0; i--) {
        microkit_dbg_putc(hex[(value >> (i * 4)) & 0xf]);
    }
}

static void check_cap(const char *name, seL4_CPtr cap)
{
    if (cap == seL4_CapNull) {
        microkit_dbg_puts("|primary  | missing cap: ");
        microkit_dbg_puts(name);
        microkit_dbg_puts("\n");
        halt();
    }
}

static void print_frame_info(const char *name, seL4_CPtr cap, seL4_Word vaddr)
{
    seL4_Word paddr;
    seL4_Error err = microkit_page_get_address(cap, &paddr);
    if (err != seL4_NoError) {
        microkit_dbg_puts("|primary  | error retrieving physical address for ");
        microkit_dbg_puts(name);
        microkit_dbg_puts("\n");
        halt();
    }

    microkit_dbg_puts("|primary  | ");
    microkit_dbg_puts(name);
    microkit_dbg_puts(" frame vaddr ");
    put_hex64(vaddr);
    microkit_dbg_puts(" paddr ");
    put_hex64(paddr);
    microkit_dbg_puts("\n");
}

static void validate_frame_metadata(void)
{
    seL4_Word secondary_stack_bottom;
    if (!microkit_root_slot_to_metadata(SLOT_SECONDARY_STACK, &secondary_stack_bottom)) {
        microkit_dbg_puts("|primary  | error retrieving secondary stack metadata\n");
        halt();
    }
    print_frame_info("secondary stack bottom", CAP_SECONDARY_STACK, secondary_stack_bottom);
    for (seL4_Word i = 1; i < STACK_SIZE / MICROKIT_BIT(seL4_PageBits); i++) {
        print_frame_info("stack frame", CAP_SECONDARY_STACK | i,
                         secondary_stack_bottom + i * MICROKIT_BIT(seL4_PageBits));
    }

    seL4_Word secondary_ipcbuf;
    if (!microkit_root_slot_to_metadata(SLOT_SECONDARY_IPCBUF, &secondary_ipcbuf)) {
        microkit_dbg_puts("|primary  | error retrieving secondary IPC buffer metadata\n");
        halt();
    }
    print_frame_info("secondary IPC buffer", CAP_SECONDARY_IPCBUF, secondary_ipcbuf);

    seL4_Word secondary_elf_metadata;
    if (!microkit_root_slot_to_metadata(SLOT_SECONDARY_ELF, &secondary_elf_metadata)) {
        microkit_dbg_puts("|primary  | error retrieving secondary ELF metadata\n");
        halt();
    }
    microkit_dbg_puts("|primary  | secondary ELF metadata at ");
    put_hex64(secondary_elf_metadata);
    microkit_dbg_puts("\n");

    seL4_Word secondary_elf_frame_count = 0;
    for (seL4_Word i = 0;; i++) {
        seL4_Word secondary_elf_frame_vaddr;
        seL4_Bool found =
            microkit_root_slot_to_nested_metadata(SLOT_SECONDARY_ELF, i, &secondary_elf_frame_vaddr);
        if (!found) {
            break;
        }

        print_frame_info("secondary ELF", CAP_SECONDARY_ELF | i, secondary_elf_frame_vaddr);
        secondary_elf_frame_count++;
    }
    if (secondary_elf_frame_count == 0) {
        microkit_dbg_puts("|primary  | error: secondary ELF metadata has no frame entries\n");
        halt();
    }
}

static void validate_mr_frame_caps(void)
{
    for (seL4_Word i = 0; i < MR_SIZE / MR_PAGE_SIZE; i++) {
        seL4_CPtr frame = CAP_MR | i;
        seL4_Word paddr;
        seL4_Error err = microkit_page_get_address(frame, &paddr);
        if (err != seL4_NoError) {
            microkit_dbg_puts("|primary  | error invoking MR frame capability\n");
            halt();
        }

        microkit_dbg_puts("|primary  | dma_buffer frame ");
        put_hex64(i);
        microkit_dbg_puts(" paddr ");
        put_hex64(paddr);
        microkit_dbg_puts("\n");

        err = microkit_page_map(frame, CAP_MY_VSPACE, DMA_BUFFER_VADDR + i * MR_PAGE_SIZE, seL4_ReadWrite,
                                MICROKIT_EXAMPLE_DEFAULT_VM_ATTRIBUTES);
        if (err != seL4_NoError) {
            microkit_dbg_puts("|primary  | error mapping MR frame into VSpace\n");
            halt();
        }

        volatile uint8_t *buf = (volatile uint8_t *)(DMA_BUFFER_VADDR + i * MR_PAGE_SIZE);
        *buf = (uint8_t)(i + 1);

        err = microkit_page_unmap(frame);
        if (err != seL4_NoError) {
            microkit_dbg_puts("|primary  | error unmapping MR frame from VSpace\n");
            halt();
        }

#if defined(CONFIG_ARCH_X86_64) && defined(CONFIG_IOMMU)
        err = microkit_io_page_map(frame, CAP_IOSPACE, seL4_ReadWrite, RUNTIME_IOVA + i * MR_PAGE_SIZE);
        if (err != seL4_NoError) {
            microkit_dbg_puts("|primary  | error mapping MR frame into IOSpace\n");
            halt();
        }

        err = microkit_page_unmap(frame);
        if (err != seL4_NoError) {
            microkit_dbg_puts("|primary  | error unmapping MR frame from IOSpace\n");
            halt();
        }
#endif
    }
}

void init(void)
{
    seL4_Error err;

    microkit_dbg_puts("|primary  | hello, world\n");

    check_cap("secondary SC", CAP_SECONDARY_SC);
    check_cap("secondary TCB", CAP_SECONDARY_TCB);
    check_cap("my SC", CAP_MY_SC);
    check_cap("my TCB", CAP_MY_TCB);
    check_cap("my VSpace", CAP_MY_VSPACE);
    check_cap("dma_buffer frames", CAP_MR);
    check_cap("QEMU EDU IOSpace", CAP_IOSPACE);
    check_cap("secondary stack frames", CAP_SECONDARY_STACK);
    check_cap("secondary IPC buffer frame", CAP_SECONDARY_IPCBUF);
    check_cap("secondary ELF frames", CAP_SECONDARY_ELF);

    validate_frame_metadata();
    validate_mr_frame_caps();

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

    microkit_dbg_puts("|primary  | halting (success)...\n");
    halt();
}

void notified(microkit_channel ch)
{
}
