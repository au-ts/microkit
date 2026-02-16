#include "elf_loader.h"
#include <string.h>

#define PAGE_SIZE 4096

int elf_validate(const void *elf_blob)
{

    const elfHeader64 *hdr = (const elfHeader64 *)elf_blob;

    if (memcmp(hdr, (const void *)ELF_MAGIC, 3) != 0)
    {
        return -1;
    }

    // if (hdr->class != 2)
    // {
    //     return -1;
    // }

    // if (hdr->data != 1)
    // {
    //     return -1;
    // }

    return 0;
}

/*
    Read a program header at a given offset from the elf blob into a
    pointer passed in
*/
size_t elf_load_program_header(const void *elf_blob, size_t index, elfHeader64 hdr, elfProgramHeader64 *dest)
{
    size_t offset = hdr.phoff + index * hdr.phentsize;
    memcpy(dest, elf_blob + offset, sizeof(elfProgramHeader64));
    return sizeof(elfProgramHeader64);
}

/**
 * Map segment pages into a child PD's vspace.
 *
 * parameters:
 *   untyped_cap: controller's untyped memory cap (for creating frames)
 *   vspace_cap: child PD's VSpace (page table) capability
 *   segment_data: pointer to segment data in memory
 *   segment_size: size of segment data in bytes
 *   vaddr: virtual address where segment should be mapped
 *   flags: ELF segment flags (PF_R, PF_W, PF_X)
 */
int map_segment_pages_with_frames(
    seL4_CPtr frame_cap,
    seL4_CPtr vspace_cap,
    seL4_CPtr controller_vspace_cap,
    seL4_CPtr controller_ut_slot,
    const void *segment_data,
    size_t segment_size,
    uint64_t vaddr,
    uint32_t elf_flags)
{
    if (vspace_cap == 0 || segment_data == NULL)
    {
        return -1;
    }

    uint64_t cur_vaddr = vaddr & ~(PAGE_SIZE - 1);
    size_t bytes_remaining = segment_size;
    size_t data_offset = 0;

    int readable = (elf_flags & PF_R) ? 1 : 0;
    int writable = (elf_flags & PF_W) ? 1 : 0;
    // int executable = (elf_flags & PF_X) ? 1 : 0;

    while (bytes_remaining > 0)
    {
        size_t bytes_to_copy = (bytes_remaining < PAGE_SIZE) ? bytes_remaining : PAGE_SIZE;
        uint64_t controller_vspace_addr = 0x5000000UL;

        // we first map into the controller vspace so we can write elf data to it
        seL4_CapRights_t controller_rights = seL4_CapRights_new(0, 0, 1, 1);
        seL4_ARM_Page_Map(frame_cap, controller_vspace_cap, controller_vspace_addr, controller_rights, 0);

        // wipe all the old data off
        memset((void *)controller_vspace_addr, 0, PAGE_SIZE);

        // copy in new data from ELF file
        memcpy((void *)controller_vspace_addr, (uint8_t *)segment_data + data_offset, bytes_to_copy);

        seL4_CapRights_t rights = seL4_CapRights_new(0, 0, readable, writable);
        seL4_ARM_Page_Map(frame_cap, vspace_cap, cur_vaddr, rights, 0);

        // unmap from the controller PD
        seL4_ARM_Page_Unmap(frame_cap);

        // Move to next page
        cur_vaddr += PAGE_SIZE;
        data_offset += bytes_to_copy;
        bytes_remaining -= bytes_to_copy;
    }

    return 0;
}
