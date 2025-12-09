/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <microkit.h>

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

void notified(microkit_channel ch)
{
}