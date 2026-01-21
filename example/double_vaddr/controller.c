/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>
#include <string.h>
#include "elf_loader.h"

/*
    Flow:
    1. PD 1 executes
    2. PD 1 notifies the controller
    3. controller pd switches the context of PD 1 to PD secondary
    4. run pd secondary using thread switch.
*/

uintptr_t pd_1_cnode_addr;
uintptr_t pd_1_vnode_addr;
uintptr_t pd_1_tcb_cap;
uintptr_t pd_1_entry_point;

uintptr_t pd_2_cnode_addr;
uintptr_t pd_2_vnode_addr;
uintptr_t pd_2_tcb_cap;
uintptr_t pd_2_entry_point;

uintptr_t fault_ep_addr;

// memory region to map ELF into
#define ELF_BLOB_ADDR 0x10000000

#define PD_CONTROLLER_UT_CAP_SLOT 200
#define PD_CONTROLLER_ASID_CAP_SLOT 201
#define PD_CONTROLLER_PAGE_SIZE_BITS 21

#define MAX_FRAMES_PER_CHILD 512
#define MAX_PDS 63

#define ELF_SIZE_OFFSET ELF_BLOB_ADDR
#define ELF_START_OFFSET ELF_BLOB_ADDR + 8

// struct to keep track of which childrens' frames we have access to
typedef struct
{
    uint64_t pd_id;
    uint64_t frame_cap_slots[MAX_FRAMES_PER_CHILD];
    uint64_t frame_cap_count;
    uint64_t child_vaddr_base;
} ChildPDFrameCapSlots;

typedef struct
{
    ChildPDFrameCapSlots children[MAX_PDS];
} AllChildFrameCapSlots;

__attribute__((section(".data")))
__attribute__((used))
AllChildFrameCapSlots __child_frame_cap_slots = {0};

void init(void)
{
    // seL4_TCB_Configure()
}

void do_switch()
{
    // access tcb of double elf
    // change the vnode root and cnode root

    //     A newly created thread is initially inactive. It is configured by setting its CSpace and VSpace
    // with the seL4_TCB_SetSpace() or seL4_TCB_Configure() methods and then calling seL4_TCB_-
    // WriteRegisters() with an initial stack pointer and instruction pointer. The thread can then be
    // activated either by setting the resume_target parameter in the seL4_TCB_WriteRegisters() invocation to true or by separately calling the seL4_TCB_Resume() method. Both of these methods
    // place the thread in a runnable state

    // setvar adds a symbol with the name of the vaddr into the elf file -> make a region with vaddr of caps (?)
    seL4_Error err;
    seL4_UserContext con = {0};
    con.pc = pd_2_entry_point;

    microkit_dbg_puts("CONTROLLER: attempting setspace. Types are below:\n");

    for (int i = 0; i < 255; i++)
    {
        int index = seL4_DebugCapIdentify(i);
        microkit_dbg_put32(i);
        microkit_dbg_puts(": ");
        microkit_dbg_put32(index);
        microkit_dbg_puts("\n");
    }

    int type = seL4_DebugCapIdentify(pd_1_tcb_cap);
    microkit_dbg_put32(type);
    microkit_dbg_puts("\n");

    type = seL4_DebugCapIdentify(pd_2_cnode_addr);
    microkit_dbg_put32(type);
    microkit_dbg_puts("\n");

    type = seL4_DebugCapIdentify(pd_2_vnode_addr);

    microkit_dbg_put32(type);
    microkit_dbg_puts("\n");

    // err = seL4_TCB_SetSpace(
    //     pd_1_tcb_cap,
    //     fault_ep_addr,
    //     pd_2_cnode_addr,
    //     0,
    //     pd_2_vnode_addr,
    //     0);

    err = seL4_TCB_SetSpace(
        203,
        212,
        207,
        0,
        210,
        0);

    if (err != seL4_NoError)
    {
        microkit_dbg_puts("microkit_pd_restart: error writing TCB caps\n");
        microkit_internal_crash(err);
    }

    microkit_dbg_puts("CONTROLLER: attempting register edit\n");

    // err = seL4_TCB_WriteRegisters(
    //     pd_2_tcb_cap,
    //     seL4_True,
    //     0,
    //     1, // writing only one register
    //     &con);

    err = seL4_TCB_WriteRegisters(
        203,
        seL4_True,
        0,
        1, // writing only one register
        &con);

    if (err != seL4_NoError)
    {
        microkit_dbg_puts("microkit_pd_restart: error writing TCB registers\n");
        microkit_internal_crash(err);
    }

    microkit_dbg_puts("sched dump controller\n");
    seL4_DebugDumpScheduler();
}

seL4_MessageInfo_t protected(microkit_channel ch, microkit_msginfo msginfo)
{
    switch (microkit_msginfo_get_label(msginfo))
    {
    case 1:
        // recieve notification from main pd
        // respond by switching the context of that pd
        microkit_dbg_puts("CONTROLLER: RECEIVED SIGNAL FROM INITIAL PD: tcb, cnode, vnode, entry\n");

        microkit_dbg_put32(pd_1_tcb_cap);
        microkit_dbg_puts("\n");

        microkit_dbg_put32(pd_1_cnode_addr);
        microkit_dbg_puts("\n");

        microkit_dbg_put32(pd_1_vnode_addr);
        microkit_dbg_puts("\n");

        microkit_dbg_put32(pd_1_entry_point);
        microkit_dbg_puts("\n");

        microkit_dbg_puts("CONTROLLER: SWITCHING TO: tcb, cnode, vnode, entry\n");

        microkit_dbg_put32(pd_2_tcb_cap);
        microkit_dbg_puts("\n");

        microkit_dbg_put32(pd_2_cnode_addr);
        microkit_dbg_puts("\n");

        microkit_dbg_put32(pd_2_vnode_addr);
        microkit_dbg_puts("\n");

        microkit_dbg_put32(pd_2_entry_point);
        microkit_dbg_puts("\n");

        do_switch();
        break;
    default:
        microkit_dbg_puts("ERROR: received an unexpected message\n");
    }
    return microkit_msginfo_new(0, 0);
}

// Unmap all frames for a pd's vspace, so that they can be
// rewritten with new elf file contents
void unmap_child_pd_frames(int child_pd_id)
{
    if (child_pd_id < 0 || child_pd_id >= MAX_PDS)
    {
        microkit_dbg_puts("unmap_child_pd_frames: invalid PD ID\n");
        return;
    }

    ChildPDFrameCapSlots *child = &__child_frame_cap_slots.children[child_pd_id];

    if (child->frame_cap_count == 0)
    {
        microkit_dbg_puts("unmap_child_pd_frames: no frames to unmap\n");
        return;
    }

    microkit_dbg_puts("Unmapping ");

    char buf[32];
    for (int i = 0; i < 20 && i < child->frame_cap_count; i++)
    {
        buf[i] = '0' + (child->frame_cap_count / 10);
    }

    microkit_dbg_puts(buf);
    microkit_dbg_puts("frames for PD incoming");
    buf[0] = '0' + child_pd_id;
    buf[1] = '\0';
    microkit_dbg_puts(buf);
    microkit_dbg_puts("\n");

    for (uint64_t i = 0; i < child->frame_cap_count; i++)
    {
        seL4_CPtr frame_cap = child->frame_cap_slots[i];
        int ret = seL4_ARM_Page_Unmap(frame_cap);

        if (ret != seL4_NoError)
        {
            microkit_dbg_puts("WARNING: failed to unmap frame cap ");
            microkit_dbg_puts("\n");
        }
    }
}

seL4_Word vspace_init(int elf_index)
{
    // step 0: verify that the elf is valid
    // step 1: read the first elf header
    // find how many program headers are in the elf file
    // then for each program header load them into a region (use the loop index to get the offset)

    elfHeader64 elf_header;

    if (elf_validate(ELF_START_OFFSET) != 0)
    {
        // bad elf file
        return 0;
    }

    // copy in the header
    memcpy(&elf_header, ELF_START_OFFSET, sizeof(elfHeader64));

    int segment_count = elf_header.phnum;
    size_t offset = 0;
    int frame_cap_count = 0;

    for (int i = 0; i < segment_count; i++)
    {
        elfProgramHeader64 cur_pheader;
        offset += elf_load_program_header(ELF_START_OFFSET, i, elf_header, &cur_pheader);

        // skip non-loadable segments
        if (cur_pheader.type_ != 1)
        {
            continue;
        }

        map_segment_pages_with_frames(
            PD_CONTROLLER_UT_CAP_SLOT + frame_cap_count,
            pd_1_vnode_addr,
            // controller vnode address,
            ELF_START_OFFSET + cur_pheader.offset,
            cur_pheader.filesz,
            cur_pheader.vaddr,
            cur_pheader.flags);
    }
}

void notified(microkit_channel ch)
{
}