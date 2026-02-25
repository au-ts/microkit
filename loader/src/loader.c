/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include "loader.h"

#include <kernel/gen_config.h>

#include "arch.h"
#include "cpus.h"
#include "cutil.h"
#include "uart.h"

_Static_assert(sizeof(uintptr_t) == 8 || sizeof(uintptr_t) == 4, "Expect uintptr_t to be 32-bit or 64-bit");

#if UINTPTR_MAX == 0xffffffffUL
#define WORD_SIZE 32
#else
#define WORD_SIZE 64
#endif

#if WORD_SIZE == 32
#define MAGIC 0x5e14dead
#else
#define MAGIC 0x5e14dead14de5ead
#endif

#define STACK_SIZE 4096

#define REGION_TYPE_DATA 1
#define REGION_TYPE_ZERO 2

#define FLAG_SEL4_HYP (1UL << 0)

#if defined(NUM_MULTIKERNELS) && NUM_MULTIKERNELS > 1
uint64_t num_multikernels = NUM_MULTIKERNELS;
 int core_up[NUM_MULTIKERNELS];
#else
uint64_t num_multikernels = 1;
#endif

#if defined(NUM_MULTIKERNELS) && NUM_MULTIKERNELS > 1
uint64_t _stack[NUM_MULTIKERNELS][STACK_SIZE] ALIGN(16);
#else
uint64_t _stack[NUM_ACTIVE_CPUS][STACK_SIZE / sizeof(uint64_t)];
#endif

typedef void (*sel4_entry)(uintptr_t kernel_boot_info_p);

#if defined(ARCH_aarch64)
static inline void dsb(void)
{
    asm volatile("dsb sy" ::: "memory");
}
#endif

extern char _text;
extern char _bss_end;
struct loader_data *loader_data = (void *) &_bss_end;
struct region *regions; // Should be end of loader data at loader_data->kernel_data[laoder_data->num_kernels]

/*
 * Print out the loader data structure.
 *
 * This doesn't *do anything*. It helps when
 * debugging to verify that the data structures are
 * being interpreted correctly by the loader.
 */
static void print_flags(void)
{
    if (is_set(CONFIG_ARM_HYPERVISOR_SUPPORT)) {
        puts("             seL4 configured as hypervisor\n");
    }
}

static uintptr_t loader_mem_start;
static uintptr_t loader_mem_end;

static void print_loader_data(void)
{
    loader_mem_start = (uintptr_t)&_text;
    loader_mem_end = (uintptr_t)&loader_data + loader_data->size;

    puts("LDR|INFO: flags:\n");
    print_flags();
    puts("LDR|INFO: Size:                 ");
    puthex64(loader_data->size);
    puts("\n");

    puts("LDR|INFO: Memory:               [");
    puthex64(loader_mem_start);
    puts("..");
    puthex64(loader_mem_end);
    puts(")\n");

    for (uint32_t i = 0; i < loader_data->num_kernels; i++) {
        puts("LDR|INFO: Kernel: ");
        puthex64(i);
        puts("\n");
        if (loader_data->kernel_bootinfos_and_regions[i].info.magic != 0x73654c34) {
            puts("LDR|INFO: We have failed the magic check for kernel boot info. It is malformed: ");
            puthex64(loader_data->kernel_bootinfos_and_regions[i].info.magic);
            puts("\n");
        } else {
            puts("LDR|INFO: We have succeeded the magic check for kernel boot info. Ok to continue!\n");
        }
        puts("LDR|INFO: num kernel regions: ");
        puthex64(loader_data->kernel_bootinfos_and_regions[i].info.num_kernel_regions);
        puts("\n");
        puts("LDR|INFO: root task entry: ");
        puthex64(loader_data->kernel_bootinfos_and_regions[i].info.root_task_entry);
        puts("\n");
    }

    for (uint32_t i = 0; i < loader_data->num_regions; i++) {
        const struct region *r = &regions[i];
        puts("LDR|INFO: region: ");
        puthex32(i);
        puts("   addr: ");
        puthex64(r->load_addr);
        puts("   load size: ");
        puthex64(r->load_size);
        puts("   write size: ");
        puthex64(r->write_size);
        puts("   offset: ");
        puthex64(r->offset);
        puts("   type: ");
        puthex64(r->type);
        puts("\n");
    }
}

static void copy_data(void)
{
    const void *base = &regions[loader_data->num_regions];
    for (uint32_t i = 0; i < loader_data->num_regions; i++) {
        const struct region *r = &regions[i];

        // This should have been checked by the tool.
        if (r->load_addr >= loader_mem_start && (r->load_addr + r->write_size) < loader_mem_end) {
            puts("LDR|ERROR: data destination overlaps with loader\n");
            for (;;) {}
        }
        // XXX: assert load_size <= write_size.
        memcpy((void *)r->load_addr, base + r->offset, r->load_size);
        if (r->write_size > r->load_size) {
            // zero out remaining memory
            memzero((void *)(r->load_addr + r->load_size), r->write_size - r->load_size);
        }
    }
}

void start_kernel(int id)
{
    puts("LDR|INFO: Kernel starting: ");
    puthex64(id);
    puts("\n\thas entry point: ");
    puthex64(loader_data->kernel_v_entry);
    puts("\n");
    puts("\thas kernel_boot_info_p: ");
    puthex64((uintptr_t)&loader_data->kernel_bootinfos_and_regions[id].info);
    puts("\n");
        
    LDR_PRINT("INFO", id, "enabling MMU\n");
    int r = arch_mmu_enable(id);
    if (r != 0) {
        LDR_PRINT("ERROR", id, "failed to enable MMU: ");
        puthex32(r);
        puts("\n");
        for (;;) {}
    }
#if defined(NUM_MULTIKERNELS) && NUM_MULTIKERNELS > 1
    dsb();
    __atomic_store_n(&core_up[id], 1, __ATOMIC_RELEASE);
    dsb();
#endif
    LDR_PRINT("INFO", id, "jumping to kernel\n");

    ((sel4_entry)(loader_data->kernel_v_entry))(
        (uintptr_t)&loader_data->kernel_bootinfos_and_regions[id].info
    );
}

// Multikernel features, powers on extra cpus with their own stack and own kernel entry
#if defined(NUM_MULTIKERNELS) && NUM_MULTIKERNELS > 1

// In utils
void disable_caches_el2(void);

volatile uint64_t cpu_mpidrs[NUM_MULTIKERNELS];

#endif

void relocation_failed(void)
{
    puts("LDR|ERROR: relocation failed, loader destination would overlap current loader location\n");
    while (1);
}

void relocation_log(uint64_t reloc_addr, uint64_t curr_addr)
{
    /* This function is called from assembly before main so we call uart_init here as well. */
    uart_init();
    puts("LDR|INFO: relocating from ");
    puthex64(curr_addr);
    puts(" to ");
    puthex64(reloc_addr);
    puts("\n");
}

int main(void)
{

    uart_init();
    /* After any UART initialisation is complete, setup an arch-specific exception
     * handler in case we fault somewhere in the loader. */
    arch_set_exception_handler();

    arch_init();

    puts("LDR|INFO: altloader for seL4 starting\n");
    /* Check that the loader magic number is set correctly */
    if (loader_data->magic != MAGIC) {
        puts("LDR|ERROR: mismatch on loader data structure magic number\n");
        goto fail;
    } else {
        puts("LDR|INFO: Magic check succeeded\n");
    }

    regions = (void *) &(loader_data->kernel_bootinfos_and_regions[loader_data->num_kernels]);

    print_loader_data();

    /* past here we have trashed u-boot so any errors should go to the
     * fail label; it's not possible to return to U-boot
     */
    copy_data();

    puts("LDR|INFO: # of multikernels is ");
    puthex64(num_multikernels);
    puts("\n");

#if defined(NUM_MULTIKERNELS) && NUM_MULTIKERNELS > 1

    disable_caches_el2();

    /* Get the CPU ID of the CPU we are booting on. */
    uint64_t boot_cpu_id, mpidr_el1;
    asm volatile("mrs %x0, mpidr_el1" : "=r"(mpidr_el1) :: "cc");
    boot_cpu_id = mpidr_el1 & 0x00ffffff;
    if (boot_cpu_id >= NUM_MULTIKERNELS) {
        puts("LDR|ERROR: Boot CPU ID (");
        puthex32(boot_cpu_id);
        puts(") exceeds the maximum CPU ID expected (");
        puthex32(NUM_MULTIKERNELS - 1);
        puts(")\n");
        goto fail;
    }
    puts("LDR|INFO: Boot CPU MPIDR (");
    puthex64(mpidr_el1);
    puts(")\n");

    cpu_mpidrs[0] = mpidr_el1;

    /* Start each CPU, other than the one we are booting on. */
    for (int i = 0; i < NUM_MULTIKERNELS; i++) {
        if (i == boot_cpu_id) continue;
        puts("LDR|INFO: Starting other CPUs (");
        puthex32(i);
        puts(")\n");

        plat_start_cpu(i);
        while (!__atomic_load_n(&core_up[i], __ATOMIC_ACQUIRE));
    }

    puts("LDR|INFO: MPIDR Map:\n");
    for (int i = 0; i < NUM_MULTIKERNELS; i++) {
        puts("LDR|INFO: CPU");
        puthex64(i);
        puts(" |-> ");
        puthex64(cpu_mpidrs[i]);
        puts("\n");
    }

    for (int i = 0; i < NUM_MULTIKERNELS; i++) {
        seL4_KernelBootInfo *bootinfo = &loader_data->kernel_bootinfos_and_regions[i].info;
        void *descriptor_mem = &loader_data->kernel_bootinfos_and_regions[i].regions_memory;
        seL4_KernelBoot_KernelRegion *kernel_regions = descriptor_mem;
        seL4_KernelBoot_RamRegion *ram_regions = (void *)((uintptr_t)kernel_regions + (bootinfo->num_kernel_regions * sizeof(seL4_KernelBoot_KernelRegion)));
        seL4_KernelBoot_RootTaskRegion *root_task_regions = (void *)((uintptr_t)ram_regions + (bootinfo->num_ram_regions * sizeof(seL4_KernelBoot_RamRegion)));
        seL4_KernelBoot_ReservedRegion *reserved_regions = (void *)((uintptr_t)root_task_regions + (bootinfo->num_root_task_regions * sizeof(seL4_KernelBoot_RootTaskRegion)));
        uint64_t *mpidr_values = (void *)((uintptr_t)reserved_regions + (bootinfo->num_reserved_regions * sizeof(seL4_KernelBoot_ReservedRegion)));
        for (int j = 0; j < NUM_MULTIKERNELS; j++) {
            mpidr_values[j] = cpu_mpidrs[j];
        }
    }


#endif /* 1 || defined(NUM_MULTIKERNELS) && NUM_MULTIKERNELS > 1 */

    puts("LDR|INFO: jumping to first kernel\n");
    start_kernel(0);

    puts("LDR|ERROR: seL4 Loader: Error - KERNEL RETURNED\n");

fail:
    /* Note: can't usefully return to U-Boot once we are here. */
    /* IMPROVEMENT: use SMC SVC call to try and power-off / reboot system.
     * or at least go to a WFI loop
     */
    for (;;) {
    }
}
