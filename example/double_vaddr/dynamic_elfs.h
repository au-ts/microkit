/*
 * Dynamic ELF module system for lazy-loading PDs at runtime.
*/

#ifndef __DYNAMIC_ELFS_H__
#define __DYNAMIC_ELFS_H__

#include <stdint.h>
#include <string.h>

/* ELF header magic number */
#define ELF_MAGIC 0x464c457f 

/* Stores ELF metadata for monitor */
struct dynamic_elf_module {
    uintptr_t vaddr;  // Virtual address where ELF is mapped 
    uint32_t size;           
};

/* Array of modules - index directly corresponds to module ID */
struct dynamic_elf_library {
    uint32_t num_modules;
    struct dynamic_elf_module modules[16]; 
};

static inline int is_elf(const void *data)
{
    const uint32_t *magic = (const uint32_t *)data;
    return *magic == ELF_MAGIC;
}

/* Direct lookup by module index */
static inline struct dynamic_elf_module *get_dynamic_module(
    struct dynamic_elf_library *lib, 
    uint32_t module_id
)
{
    if (module_id >= lib->num_modules) {
        return NULL;
    }
    return &lib->modules[module_id];
}

#endif /* __DYNAMIC_ELFS_H__ */
