#pragma once

#include <stdint.h>
#include <stddef.h>
#include <sel4/sel4.h>

/**
 * ELF parsing and dynamic loading utilities in C for the controller (stolen from AOS code hahahah)
 * required for loading elf into vspace on the fly
 */

#define ELF_MAGIC 0x464c457f

typedef struct
{
    uint32_t ident_magic;
    uint8_t ident_class;
    uint8_t ident_data;
    uint8_t ident_version;
    uint8_t ident_osabi;
    uint8_t ident_abiversion;
    uint8_t _padding[7];
    uint16_t type_;
    uint16_t machine;
    uint32_t version;
    uint64_t entry;
    uint64_t phoff;
    uint64_t shoff;
    uint32_t flags;
    uint16_t ehsize;
    uint16_t phentsize;
    uint16_t phnum;
    uint16_t shentsize;
    uint16_t shnum;
    uint16_t shstrndx;
} elfHeader64;

typedef struct
{
    uint32_t type_;
    uint32_t flags;
    uint64_t offset;
    uint64_t vaddr;
    uint64_t paddr;
    uint64_t filesz;
    uint64_t memsz;
    uint64_t align;
} elfProgramHeader64;

#define PT_LOAD 1
#define PF_X 0x1
#define PF_W 0x2
#define PF_R 0x4

int elf_validate(const void *elf_blob);

size_t elf_load_program_header(const void *elf_blob, size_t index, elfHeader64 hdr, elfProgramHeader64 *dest);

int map_segment_pages_with_frames(
    seL4_CPtr untyped_cap,
    seL4_CPtr vspace_cap,
    seL4_CPtr controller_vspace_cap,
    seL4_CPtr page_table_cap,
    const void *segment_data,
    size_t segment_size,
    uint64_t vaddr,
    uint32_t elf_flags);
