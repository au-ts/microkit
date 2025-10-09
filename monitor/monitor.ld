/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
PHDRS
{
    text PT_LOAD;
    data PT_LOAD;
}

ENTRY(_start);

SECTIONS
{
    . = 0x200000;

    .text :
    {
        _text = .;
        *(.text.start)
        *(.text*)
        *(.rodata)
        _text_end = .;
    } :text

    . = ALIGN(0x1000);
    .data :
    {
        _data = .;
        *(.data)
        . = ALIGN(8);
        __global_pointer$ = . + 0x800;
        *(.srodata)
        *(.sdata)
        _data_end = .;
    } :data

    .bss :
    {
        _bss = .;
        *(.sbss)
        *(.bss)
        *(.bss*)
        *(COMMON)
        . = ALIGN(4);
        _bss_end = .;
    } :data

    . = ALIGN(0x1000);
    .ipc_buffer (NOLOAD): {
        __sel4_ipc_buffer_obj = .;
    } :NONE
}
