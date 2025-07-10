/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
PHDRS
{
    all PT_LOAD AT (0x08a000000);
}

SECTIONS
{
    . = 0x08a000000;

    .text :
    {
        _text = .;
        *(.text.start)
        *(.text*)
        *(.rodata)
        _text_end = .;
    } :all

    .data :
    {
        _data = .;
        *(.data)
        __global_pointer$ = . + 0x800;
        *(.srodata)
        *(.sdata)
        _data_end = .;
    } :all

    .bss :
    {
        _bss = .;
        *(.sbss)
        *(.bss)
        *(COMMON)
        . = ALIGN(4);
        _bss_end = .;
    } :all

    . = ALIGN(0x1000);
    .ipc_buffer (NOLOAD): {
        __sel4_ipc_buffer_obj = .;
    }
}
