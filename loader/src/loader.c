/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */
#include <stdint.h>
#include <stddef.h>

#include "kernel/gen_config.h"

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

#define ALIGN(n)  __attribute__((__aligned__(n)))

#define MASK(x) ((1U << x) - 1)

#define STACK_SIZE 4096

#define UART_REG(x) ((volatile uint32_t *)(UART_BASE + (x)))

#if defined(BOARD_zcu102) || defined(BOARD_ultra96v2)
#define GICD_BASE 0x00F9010000UL
#define GICC_BASE 0x00F9020000UL
#define GIC_VERSION 2
#elif defined(BOARD_qemu_virt_aarch64) || defined(BOARD_qemu_virt_aarch64_multikernel)
#define GICD_BASE 0x8000000UL
#define GICC_BASE 0x8010000UL
#define GIC_VERSION 2
#elif defined(BOARD_odroidc4) || defined(BOARD_odroidc4_multikernel)
#define GICD_BASE 0xffc01000UL
#define GICC_BASE 0xffc02000UL
#define GIC_VERSION 2
#elif defined(BOARD_maaxboard_multikernel)
/* reg = <0x38800000 0x10000 0x38880000 0xc0000 0x31000000 0x2000 0x31010000 0x2000 0x31020000 0x2000>; */
#define GICD_BASE 0x38800000UL /* size 0x10000 */
#define GICR_BASE 0x38880000UL /* size 0xc0000 */
#define GIC_VERSION 3
#else
/* #define GIC_VERSION */
#endif

#define REGION_TYPE_DATA 1
#define REGION_TYPE_ZERO 2

#define FLAG_SEL4_HYP (1UL << 0)

enum el {
    EL0 = 0,
    EL1 = 1,
    EL2 = 2,
    EL3 = 3,
};

struct region {
    uintptr_t load_addr; // this should be updated for subsequent regions by loader.rs
    // size of the data to load
    uintptr_t load_size;
    // size of the data to write. this is useful for zeroing out memory.
    uintptr_t write_size;
    uintptr_t offset;
    uintptr_t type;
};

#include "sel4/bootinfo.h"

struct KernelBootInfoAndRegions {
    seL4_KernelBootInfo info;
    uint8_t regions_memory[4096 - sizeof(seL4_KernelBootInfo)];
};

_Static_assert(sizeof(struct KernelBootInfoAndRegions) == 0x1000);

// Changing this structure is precarious, maybe better to wrap in NUM_MULTIKERNELS IFDEF
struct loader_data {
    uintptr_t magic;
    uintptr_t size;
    uintptr_t flags;
    uintptr_t num_kernels;
    uintptr_t num_regions;
    uintptr_t kernel_v_entry;
    struct KernelBootInfoAndRegions kernel_bootinfos_and_regions[];
};

typedef void (*sel4_entry)(uintptr_t kernel_boot_info_p);

static void *memcpy(void *dst, const void *src, size_t sz)
{
    char *dst_ = dst;
    const char *src_ = src;
    while (sz-- > 0) {
        *dst_++ = *src_++;
    }

    return dst;
}

void *memmove(void *restrict dest, const void *restrict src, size_t n)
{
    unsigned char *d = (unsigned char *)dest;
    const unsigned char *s = (const unsigned char *)src;

    /* no copying to do */
    if (d == s) {
        return dest;
    }
    /* for non-overlapping regions, just use memcpy */
    else if (s + n <= d || d + n <= s) {
        return memcpy(dest, src, n);
    }
    /* if copying from the start of s to the start of d, just use memcpy */
    else if (s > d) {
        return memcpy(dest, src, n);
    }

    /* copy from end of 's' to end of 'd' */
    size_t i;
    for (i = 1; i <= n; i++) {
        d[n - i] = s[n - i];
    }

    return dest;
}

static void memzero(void *s, size_t sz)
{
    uint8_t *p = s;
    while (sz-- > 0) {
        *p++ = 0x0;
    }
}

#if 1 || NUM_MULTIKERNELS > 1
volatile char _stack[NUM_MULTIKERNELS][STACK_SIZE] ALIGN(16);
#else
char _stack[STACK_SIZE] ALIGN(16);
#endif

#if defined(ARCH_aarch64)
void switch_to_el1(void);
void switch_to_el2(void);
void el1_mmu_enable(uint64_t *pgd_down, uint64_t *pgd_up);
void el2_mmu_enable(uint64_t *pgd_down);
extern char arm_vector_table[1];

#if 1 || NUM_MULTIKERNELS > 1
/* Paging structures for kernel mapping */
uint64_t boot_lvl0_upper[NUM_MULTIKERNELS][1 << 9] ALIGN(1 << 12);
uint64_t boot_lvl1_upper[NUM_MULTIKERNELS][1 << 9] ALIGN(1 << 12);
uint64_t boot_lvl2_upper[NUM_MULTIKERNELS][1 << 9] ALIGN(1 << 12);

/* Paging structures for identity mapping */
uint64_t boot_lvl0_lower[NUM_MULTIKERNELS][1 << 9] ALIGN(1 << 12);
uint64_t boot_lvl1_lower[NUM_MULTIKERNELS][1 << 9] ALIGN(1 << 12);

uint64_t num_multikernels = NUM_MULTIKERNELS;

#else
/* Paging structures for kernel mapping */
uint64_t boot_lvl0_upper[1 << 9] ALIGN(1 << 12);
uint64_t boot_lvl1_upper[1 << 9] ALIGN(1 << 12);
uint64_t boot_lvl2_upper[1 << 9] ALIGN(1 << 12);

/* Paging structures for identity mapping */
uint64_t boot_lvl0_lower[1 << 9] ALIGN(1 << 12);
uint64_t boot_lvl1_lower[1 << 9] ALIGN(1 << 12);

uint64_t num_multikernels = 1;
#endif

uintptr_t exception_register_state[32];

#elif defined(ARCH_riscv64)
/* Paging structures for kernel mapping */
uint64_t boot_lvl1_pt[1 << 9] ALIGN(1 << 12);
uint64_t boot_lvl2_pt[1 << 9] ALIGN(1 << 12);
/* Paging structures for identity mapping */
uint64_t boot_lvl2_pt_elf[1 << 9] ALIGN(1 << 12);
#endif

extern char _text;
extern char _bss_end;
struct loader_data *loader_data = (void *) &_bss_end;
struct region *regions; // Should be end of loader data at loader_data->kernel_data[laoder_data->num_kernels]

#if defined(BOARD_tqma8xqp1gb)
#define UART_BASE 0x5a070000
#define STAT 0x14
#define TRANSMIT 0x1c
#define STAT_TDRE (1 << 23)

static void uart_init() {}

static void putc(uint8_t ch)
{
    while (!(*UART_REG(STAT) & STAT_TDRE)) { }
    *UART_REG(TRANSMIT) = ch;
}

#elif defined(BOARD_imx8mm_evk) || defined(BOARD_imx8mp_evk)
#define UART_BASE 0x30890000
#define STAT 0x98
#define TRANSMIT 0x40
#define STAT_TDRE (1 << 14)

static void uart_init() {}

static void putc(uint8_t ch)
{
    while (!(*UART_REG(STAT) & STAT_TDRE)) { }
    *UART_REG(TRANSMIT) = ch;
}
#elif defined(BOARD_zcu102)
#define UART_BASE 0xff000000
#define UART_CHANNEL_STS_TXEMPTY 0x8
#define UART_CHANNEL_STS         0x2C
#define UART_TX_RX_FIFO          0x30

#define UART_CR             0x00
#define UART_CR_TX_EN       (1 << 4)
#define UART_CR_TX_DIS      (1 << 5)

static void uart_init(void)
{
    uint32_t ctrl = *UART_REG(UART_CR);
    ctrl |= UART_CR_TX_EN;
    ctrl &= ~UART_CR_TX_DIS;
    *UART_REG(UART_CR) = ctrl;
}

static void putc(uint8_t ch)
{
    while (!(*UART_REG(UART_CHANNEL_STS) & UART_CHANNEL_STS_TXEMPTY));
    *UART_REG(UART_TX_RX_FIFO) = ch;
}
#elif defined(BOARD_maaxboard) || defined(BOARD_imx8mq_evk) || defined(BOARD_maaxboard_multikernel)
#define UART_BASE 0x30860000
#define STAT 0x98
#define TRANSMIT 0x40
#define STAT_TDRE (1 << 14)

static void uart_init() {}

static void putc(uint8_t ch)
{
    // ensure FIFO has space
    while (!(*UART_REG(STAT) & STAT_TDRE)) { }
    *UART_REG(TRANSMIT) = ch;
}
#elif defined(BOARD_odroidc2)
#define UART_BASE 0xc81004c0
#define UART_WFIFO 0x0
#define UART_STATUS 0xC
#define UART_TX_FULL (1 << 21)

static void uart_init() {}

static void putc(uint8_t ch)
{
    while ((*UART_REG(UART_STATUS) & UART_TX_FULL));
    *UART_REG(UART_WFIFO) = ch;
}
#elif defined(BOARD_odroidc4) || defined(BOARD_odroidc4_multikernel) || defined(BOARD_odroidc4_multikernel_1) || defined(BOARD_odroidc4_multikernel_2)
#define UART_BASE 0xff803000
#define UART_WFIFO 0x0
#define UART_STATUS 0xC
#define UART_TX_FULL (1 << 21)

static void uart_init() {}

static void putc(uint8_t ch)
{
    while ((*UART_REG(UART_STATUS) & UART_TX_FULL));
    *UART_REG(UART_WFIFO) = ch;
}
#elif defined(BOARD_ultra96v2)
/* Use UART1 available through USB-to-JTAG/UART pod */
#define UART_BASE 0x00ff010000
#define R_UART_CHANNEL_STS          0x2C
#define UART_CHANNEL_STS_TXEMPTY    0x08
#define UART_CHANNEL_STS_TACTIVE    0x800
#define R_UART_TX_RX_FIFO           0x30

static void uart_init(void) {}

static void putc(uint8_t ch)
{
    while (!(*UART_REG(R_UART_CHANNEL_STS) & UART_CHANNEL_STS_TXEMPTY)) {};
    while (*UART_REG(R_UART_CHANNEL_STS) & UART_CHANNEL_STS_TACTIVE) {};

    *((volatile uint32_t *)(UART_BASE + R_UART_TX_RX_FIFO)) = ch;
}
#elif defined(BOARD_qemu_virt_aarch64) || defined(BOARD_qemu_virt_aarch64_multikernel)
#define UART_BASE                 0x9000000
#define PL011_TCR                 0x030
#define PL011_UARTDR              0x000
#define PL011_UARTFR              0x018
#define PL011_UARTFR_TXFF         (1 << 5)
#define PL011_CR_UART_EN          (1 << 0)
#define PL011_CR_TX_EN            (1 << 8)

static void uart_init()
{
    /* Enable the device and transmit */
    *UART_REG(PL011_TCR) |= (PL011_CR_TX_EN | PL011_CR_UART_EN);
}

static void putc(uint8_t ch)
{
    while ((*UART_REG(PL011_UARTFR) & PL011_UARTFR_TXFF) != 0);
    *UART_REG(PL011_UARTDR) = ch;
}

#elif defined(BOARD_rpi4b_1gb) || defined(BOARD_rpi4b_2gb) || defined(BOARD_rpi4b_4gb) || defined(BOARD_rpi4b_8gb)
#define UART_BASE 0xfe215040
#define MU_IO 0x00
#define MU_LSR 0x14
#define MU_LSR_TXIDLE (1 << 6)

static void uart_init() {}

static void putc(uint8_t ch)
{
    while (!(*UART_REG(MU_LSR) & MU_LSR_TXIDLE));
    *UART_REG(MU_IO) = (ch & 0xff);
}
#elif defined(BOARD_rockpro64)
#define UART_BASE   0xff1a0000
#define UTHR        0x0
#define ULSR        0x14
#define ULSR_THRE   (1 << 5)

static void uart_init() {}

static void putc(uint8_t ch)
{
    while ((*UART_REG(ULSR) & ULSR_THRE) == 0);
    *UART_REG(UTHR) = ch;
}

#elif defined(ARCH_riscv64)
#define SBI_CONSOLE_PUTCHAR 1

// TODO: remove, just do straight ASM
#define SBI_CALL(which, arg0, arg1, arg2) ({            \
    register uintptr_t a0 asm ("a0") = (uintptr_t)(arg0);   \
    register uintptr_t a1 asm ("a1") = (uintptr_t)(arg1);   \
    register uintptr_t a2 asm ("a2") = (uintptr_t)(arg2);   \
    register uintptr_t a7 asm ("a7") = (uintptr_t)(which);  \
    asm volatile ("ecall"                   \
              : "+r" (a0)               \
              : "r" (a1), "r" (a2), "r" (a7)        \
              : "memory");              \
    a0;                         \
})

#define SBI_CALL_1(which, arg0) SBI_CALL(which, arg0, 0, 0)

static void uart_init()
{
    /* Nothing to do, OpenSBI will do UART init for us. */
}

static void putc(uint8_t ch)
{
    SBI_CALL_1(SBI_CONSOLE_PUTCHAR, ch);
}
#else
#error Board not defined
#endif

static void puts(const char *s)
{
#if PRINTING
    while (*s) {
        if (*s == '\n') {
            putc('\r');
        }
        putc(*s);
        s++;
    }
#endif
}

static char hexchar(unsigned int v)
{
    return v < 10 ? '0' + v : ('a' - 10) + v;
}

static void puthex32(uint32_t val)
{
    char buffer[8 + 3];
    buffer[0] = '0';
    buffer[1] = 'x';
    buffer[8 + 3 - 1] = 0;
    for (unsigned i = 8 + 1; i > 1; i--) {
        buffer[i] = hexchar(val & 0xf);
        val >>= 4;
    }
    puts(buffer);
}

static void puthex64(uint64_t val)
{
    char buffer[16 + 3];
    buffer[0] = '0';
    buffer[1] = 'x';
    buffer[16 + 3 - 1] = 0;
    for (unsigned i = 16 + 1; i > 1; i--) {
        buffer[i] = hexchar(val & 0xf);
        val >>= 4;
    }
    puts(buffer);
}

#ifdef ARCH_aarch64
static void puthex(uintptr_t val)
{
#if WORD_SIZE == 32
    puthex32(val);
#else
    puthex64(val);
#endif
}

/* Returns the current exception level */
static enum el current_el(void)
{
    /* See: C5.2.1 CurrentEL */
    uint32_t val;
    asm volatile("mrs %x0, CurrentEL" : "=r"(val) :: "cc");
    /* bottom two bits are res0 */
    return (enum el) val >> 2;
}

static char *el_to_string(enum el el)
{
    switch (el) {
    case EL0:
        return "EL0";
    case EL1:
        return "EL1";
    case EL2:
        return "EL2";
    case EL3:
        return "EL3";
    }

    return "<invalid el>";
}

static char *ex_to_string(uintptr_t ex)
{
    switch (ex) {
    case 0:
        return "Synchronous (Current Exception level with SP_EL0)";
    case 1:
        return "IRQ (Current Exception level with SP_EL0)";
    case 2:
        return "FIQ (Current Exception level with SP_EL0)";
    case 3:
        return "SError (Current Exception level with SP_EL0)";
    case 4:
        return "Synchronous (Current Exception level with SP_ELx)";
    case 5:
        return "IRQ (Current Exception level with SP_ELx)";
    case 6:
        return "FIQ (Current Exception level with SP_ELx)";
    case 7:
        return "SError (Current Exception level with SP_ELx)";
    case 8:
        return "Synchronous 64-bit EL0";
    case 9:
        return "IRQ 64-bit EL0";
    case 10:
        return "FIQ 64-bit EL0";
    case 11:
        return "SError 64-bit EL0";
    case 12:
        return "Synchronous 32-bit EL0";
    case 13:
        return "IRQ 32-bit EL0";
    case 14:
        return "FIQ 32-bit EL0";
    case 15:
        return "SError 32-bit EL0";
    }
    return "<invalid ex>";
}

static char *ec_to_string(uintptr_t ec)
{
    switch (ec) {
    case 0:
        return "Unknown reason";
    case 1:
        return "Trapped WFI or WFE instruction execution";
    case 3:
        return "Trapped MCR or MRC access with (coproc==0b1111) this is not reported using EC 0b000000";
    case 4:
        return "Trapped MCRR or MRRC access with (coproc==0b1111) this is not reported using EC 0b000000";
    case 5:
        return "Trapped MCR or MRC access with (coproc==0b1110)";
    case 6:
        return "Trapped LDC or STC access";
    case 7:
        return "Access to SVC, Advanced SIMD or floating-point functionality trapped";
    case 12:
        return "Trapped MRRC access with (coproc==0b1110)";
    case 13:
        return "Branch Target Exception";
    case 17:
        return "SVC instruction execution in AArch32 state";
    case 21:
        return "SVC instruction execution in AArch64 state";
    case 24:
        return "Trapped MSR, MRS or System instruction exuection in AArch64 state, this is not reported using EC 0xb000000, 0b000001 or 0b000111";
    case 25:
        return "Access to SVE functionality trapped";
    case 28:
        return "Exception from a Pointer Authentication instruction authentication failure";
    case 32:
        return "Instruction Abort from a lower Exception level";
    case 33:
        return "Instruction Abort taken without a change in Exception level";
    case 34:
        return "PC alignment fault exception";
    case 36:
        return "Data Abort from a lower Exception level";
    case 37:
        return "Data Abort taken without a change in Exception level";
    case 38:
        return "SP alignment faultr exception";
    case 40:
        return "Trapped floating-point exception taken from AArch32 state";
    case 44:
        return "Trapped floating-point exception taken from AArch64 state";
    case 47:
        return "SError interrupt";
    case 48:
        return "Breakpoint exception from a lower Exception level";
    case 49:
        return "Breakpoint exception taken without a change in Exception level";
    case 50:
        return "Software Step exception from a lower Exception level";
    case 51:
        return "Software Step exception taken without a change in Exception level";
    case 52:
        return "Watchpoint exception from a lower Exception level";
    case 53:
        return "Watchpoint exception taken without a change in Exception level";
    case 56:
        return "BKPT instruction execution in AArch32 state";
    case 60:
        return "BRK instruction execution in AArch64 state";
    }
    return "<invalid EC>";
}
#endif

/*
 * Print out the loader data structure.
 *
 * This doesn't *do anything*. It helps when
 * debugging to verify that the data structures are
 * being interpreted correctly by the loader.
 */
static void print_flags(void)
{
    if (loader_data->flags & FLAG_SEL4_HYP) {
        puts("             seL4 configured as hypervisor\n");
    }
}

static void print_loader_data(void)
{
    puts("LDR|INFO: Flags:                ");
    puthex64(loader_data->flags);
    puts("\n");
    print_flags();

    for (uint32_t i = 0; i < loader_data->num_kernels; i++) {
        puts("LDR|INFO: Kernel: ");
        puthex64(i);
        puts("\n");
        // puts("LDR|INFO: Kernel:      entry:   ");
        // puthex64(loader_data->kernel_data[i].kernel_entry);
        // puts("\n");
        // puts("LDR|INFO: Kernel:      kernel_elf_paddr_base:   ");
        // puthex64(loader_data->kernel_data[i].kernel_elf_paddr_base);
        // puts("\n");

        // puts("LDR|INFO: Root server: physmem: ");
        // puthex64(loader_data->kernel_data[i].ui_p_reg_start);
        // puts(" -- ");
        // puthex64(loader_data->kernel_data[i].ui_p_reg_end);
        // puts("\nLDR|INFO:              virtmem: ");
        // puthex64(loader_data->kernel_data[i].ui_p_reg_start - loader_data->kernel_data[i].pv_offset);
        // puts(" -- ");
        // puthex64(loader_data->kernel_data[i].ui_p_reg_end - loader_data->kernel_data[i].pv_offset);
        // puts("\nLDR|INFO:              entry  : ");
        // puthex64(loader_data->kernel_data[i].v_entry);
        // puts("\n");

        seL4_KernelBootInfo *bootinfo = &loader_data->kernel_bootinfos_and_regions[i].info;

        void *descriptor_mem = &loader_data->kernel_bootinfos_and_regions[i].regions_memory;
        seL4_KernelBoot_KernelRegion *kernel_regions = descriptor_mem;
        seL4_KernelBoot_RamRegion *ram_regions = (void *)((uintptr_t)kernel_regions + (bootinfo->num_kernel_regions * sizeof(seL4_KernelBoot_KernelRegion)));
        seL4_KernelBoot_RootTaskRegion *root_task_regions = (void *)((uintptr_t)ram_regions + (bootinfo->num_ram_regions * sizeof(seL4_KernelBoot_RamRegion)));
        seL4_KernelBoot_ReservedRegion *reserved_regions = (void *)((uintptr_t)root_task_regions + (bootinfo->num_root_task_regions * sizeof(seL4_KernelBoot_RootTaskRegion)));

        // TODO: print.
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
        puts("LDR|INFO: copying region ");
        puthex32(i);
        puts("\n");
        // XXX: assert load_size <= write_size.
        memcpy((void *)(uintptr_t)r->load_addr, base + r->offset, r->load_size);
        if (r->write_size > r->load_size) {
            // zero out remaining memory
            memzero((void *)(r->load_addr + r->load_size), r->write_size - r->load_size);
        }
    }
}

#ifdef ARCH_aarch64
static int ensure_correct_el(void)
{
    enum el el = current_el();

    puts("LDR|INFO: CurrentEL=");
    puts(el_to_string(el));
    puts("\n");

    if (el == EL0) {
        puts("LDR|ERROR: Unsupported initial exception level\n");
        return 1;
    }

    if (el == EL3) {
        puts("LDR|INFO: Dropping from EL3 to EL2(NS)\n");
        switch_to_el2();
        puts("LDR|INFO: Dropped from EL3 to EL2(NS)\n");
        el = EL2;
    }

    if (loader_data->flags & FLAG_SEL4_HYP) {
        if (el != EL2) {
            puts("LDR|ERROR: seL4 configured as a hypervisor, but not in EL2\n");
            return 1;
        } else {
            puts("LDR|INFO: Resetting CNTVOFF\n");
            asm volatile("msr cntvoff_el2, xzr");
        }
    } else {
        if (el == EL2) {
            /* seL4 relies on the timer to be set to a useful value */
            puts("LDR|INFO: Resetting CNTVOFF\n");
            asm volatile("msr cntvoff_el2, xzr");
            puts("LDR|INFO: Dropping from EL2 to EL1\n");
            switch_to_el1();
            puts("LDR|INFO: CurrentEL=");
            el = current_el();
            puts(el_to_string(el));
            puts("\n");
            if (el == EL1) {
                puts("LDR|INFO: Dropped to EL1 successfully\n");
            } else {
                puts("LDR|ERROR: Failed to switch to EL1\n");
                return 1;
            }
        }
    }

    return 0;
}
#endif

static void start_kernel(int id)
{
    // puts("LDR|INFO: Initial task ");
    // putc(id + '0');
    // puts(" has offset of ");
    // puthex64(loader_data->kernel_data[id].pv_offset);
    // puts(" (-");
    // puthex64(-loader_data->kernel_data[id].pv_offset);
    // puts(")\n");

    puts("LDR|INFO: Kernel starting: ");
    putc(id + '0');
    puts("\n\thas entry point: ");
    puthex64(loader_data->kernel_v_entry);
    puts("\n");
    puts("\thas kernel_boot_info_p: ");
    puthex64((uintptr_t)&loader_data->kernel_bootinfos_and_regions[id].info);
    puts("\n");
        
    ((sel4_entry)(loader_data->kernel_v_entry))(
        (uintptr_t)&loader_data->kernel_bootinfos_and_regions[id].info
    );
}

#if defined(GIC_VERSION)
#if GIC_VERSION == 2

#define IRQ_SET_ALL 0xffffffff
#define TARGET_CPU_ALLINT(CPU) ( \
        ( ((CPU)&0xff)<<0u  ) |\
        ( ((CPU)&0xff)<<8u  ) |\
        ( ((CPU)&0xff)<<16u ) |\
        ( ((CPU)&0xff)<<24u ) \
    )

/* Memory map for GICv1/v2 distributor */
struct gic_dist_map {
    uint32_t CTLR;                  /* 0x000 Distributor Control Register (RW) */
    uint32_t TYPER;                 /* 0x004 Interrupt Controller Type Register (RO) */
    uint32_t IIDR;                  /* 0x008 Distributor Implementer Identification Register (RO) */
    uint32_t _res1[29];             /* 0x00C--0x07C */
    uint32_t IGROUPRn[32];          /* 0x080--0x0FC Interrupt Group Registers (RW) */

    uint32_t ISENABLERn[32];        /* 0x100--0x17C Interrupt Set-Enable Registers (RW) */
    uint32_t ICENABLERn[32];        /* 0x180--0x1FC Interrupt Clear-Enable Registers (RW)*/

    uint32_t ISPENDRn[32];          /* 0x200--0x27C Interrupt Set-Pending Registers (RW) */
    uint32_t ICPENDRn[32];          /* 0x280--0x2FC Interrupt Clear-Pending Registers (RW) */

    uint32_t ISACTIVERn[32];        /* 0x300--0x37C GICv2 Interrupt Set-Active Registers (RW) */
    uint32_t ICACTIVERn[32];        /* 0x380--0x3FC GICv2 Interrupt Clear-Active Registers (RW) */

    uint32_t IPRIORITYRn[255];      /* 0x400--0x7F8 Interrupt Priority Registers (RW) */
    uint32_t _res3;                 /* 0x7FC */

    uint32_t ITARGETSRn[255];       /* 0x800--0xBF8 Interrupt Processor Targets Registers (RO) */
    uint32_t _res4;                 /* 0xBFC */

    uint32_t ICFGRn[64];            /* 0xC00--0xCFC Interrupt Configuration Registers (RW) */

    uint32_t _res5[64];             /* 0xD00--0xDFC IMPLEMENTATION DEFINED registers */

    uint32_t NSACRn[64];            /* 0xE00--0xEFC GICv2 Non-secure Access Control Registers, optional (RW) */

    uint32_t SGIR;                  /* 0xF00 Software Generated Interrupt Register (WO) */
    uint32_t _res6[3];              /* 0xF04--0xF0C */
    uint32_t CPENDSGIRn[4];         /* 0xF10--0xF1C GICv2 SGI Clear-Pending Registers (RW) */
    uint32_t SPENDSGIRn[4];         /* 0xF20--0xF2C GICv2 SGI Set-Pending Registers (RW) */
    uint32_t _res7[40];             /* 0xF30--0xFCC */

    // These are actually defined as "ARM implementation of the GIC Identificiation Registers" (p4-120)
    // but we never read them so let's just marked them as implementation defined.
    uint32_t _res8[6];              /* 0xFD0--0xFE4 IMPLEMENTATION DEFINED registers (RO) */
    uint32_t ICPIDR2;               /* 0xFE8 Peripheral ID2 Register (RO) */
    uint32_t _res9[5];              /* 0xFEC--0xFFC IMPLEMENTATION DEFINED registers (RO) */
};

_Static_assert(__builtin_offsetof(struct gic_dist_map, IGROUPRn) == 0x080);
_Static_assert(__builtin_offsetof(struct gic_dist_map, IPRIORITYRn) == 0x400);
_Static_assert(__builtin_offsetof(struct gic_dist_map, ICFGRn) == 0xC00);
_Static_assert(__builtin_offsetof(struct gic_dist_map, NSACRn) == 0xE00);
_Static_assert(__builtin_offsetof(struct gic_dist_map, SGIR) == 0xF00);
_Static_assert(__builtin_offsetof(struct gic_dist_map, _res8) == 0xFD0);
_Static_assert(__builtin_offsetof(struct gic_dist_map, ICPIDR2) == 0xFE8);


#define _macrotest_1 ,
#define is_set(value) _is_set__(_macrotest_##value)
#define _is_set__(comma) _is_set___(comma 1, 0)
#define _is_set___(_, v, ...) v

static uint8_t infer_cpu_gic_id(int nirqs)
{
   volatile struct gic_dist_map *gic_dist = (volatile void *)(GICD_BASE);

    uint64_t i;
    uint32_t target = 0;
    for (i = 0; i < nirqs; i += 4) {
        target = gic_dist->ITARGETSRn[i >> 2];
        target |= target >> 16;
        target |= target >> 8;
        if (target) {
            break;
        }
    }
    if (!target) {
        puts("Warning: Could not infer GIC interrupt target ID, assuming 0.\n");
        target = 0 << 1;
    }
    return target & 0xff;
}

static void configure_gicv2(void)
{
    /* The ZCU102 start in EL3, and then we drop to EL1(NS).
     *
     * The GICv2 supports security extensions (as does the CPU).
     *
     * The GIC sets any interrupt as either Group 0 or Group 1.
     * A Group 0 interrupt can only be configured in secure mode,
     * while Group 1 interrupts can be configured from non-secure mode.
     *
     * As seL4 runs in non-secure mode, and we want seL4 to have
     * the ability to configure interrupts, at this point we need
     * to put all interrupts into Group 1.
     *
     * GICD_IGROUPn starts at offset 0x80.
     *
     * 0xF901_0000.
     *
     * Future work: On multicore systems the distributor setup
     * only needs to be called once, while the GICC registers
     * should be set for each CPU.
     */
    puts("LDR|INFO: Configuring GICv2 for ARM\n");
    // -------

    volatile struct gic_dist_map *gic_dist = (volatile void *)(GICD_BASE);

    uint64_t i;
    int nirqs = 32 * ((gic_dist->TYPER & 0x1f) + 1);
    /* Bit 0 is enable; so disable */
    gic_dist->CTLR = 0;

    for (i = 0; i < nirqs; i += 32) {
        /* clear enable */
        gic_dist->ICENABLERn[i >> 5] = IRQ_SET_ALL;
        /* clear pending */
        gic_dist->ICPENDRn[i >> 5] = IRQ_SET_ALL;
    }

    /* reset interrupts priority */
    for (i = 32; i < nirqs; i += 4) {
        if (loader_data->flags & FLAG_SEL4_HYP) {
            gic_dist->IPRIORITYRn[i >> 2] = 0x80808080;
        } else {
            gic_dist->IPRIORITYRn[i >> 2] = 0;
        }
    }
    /*
     * reset int target to current cpu
     * We query which id that the GIC uses for us and use that.
     */
    uint8_t target = infer_cpu_gic_id(nirqs);
    puts("GIC target of loader: ");
    puthex32(target);
    puts("\n");

    for (i = 32; i < nirqs; i += 4) {
        /* IRQs by default target the loader's CPU, assuming it's "0" CPU interface */
        /* This gives core 0 of seL4 "permission" to configure these interrupts */
        /* cannot configure for SGIs/PPIs (irq < 32) */
        gic_dist->ITARGETSRn[i / 4] = TARGET_CPU_ALLINT(target);
        puts("gic_dist->ITARGETSRn[");
        puthex32(i);
        puts(" / 4] = ");
        puthex32(gic_dist->ITARGETSRn[i / 4]);
        puts("\n");
    }

    /* level-triggered, 1-N */
    for (i = 32; i < nirqs; i += 32) {
        gic_dist->ICFGRn[i / 32] = 0x55555555;
    }

    /* group 0 for secure; group 1 for non-secure */
    for (i = 0; i < nirqs; i += 32) {
        if (loader_data->flags & FLAG_SEL4_HYP && !is_set(BOARD_qemu_virt_aarch64)) {
            gic_dist->IGROUPRn[i / 32] = 0xffffffff;
        } else {
            gic_dist->IGROUPRn[i / 32] = 0;
        }
    }

    /* For any interrupts to go through the interrupt priority mask
     * must be set appropriately. Only interrupts with priorities less
     * than this mask will interrupt the CPU.
     *
     * seL4 (effectively) sets interrupts to priority 0x80, so it is
     * important to make sure this is greater than 0x80.
     */
    *((volatile uint32_t *)(GICC_BASE + 0x4)) = 0xf0;


    /* BIT 0 is enable; so enable */
    gic_dist->CTLR = 1;
}

#elif GIC_VERSION == 3

static void configure_gicv3(void)
{
    puts("TODO: configure gicv3\n");
}

#else
#error "unknown GIC version"
#endif

/* not a GIC */

#endif

#ifdef ARCH_riscv64

/*
 * This is the encoding for the MODE field of the satp register when
 * implementing 39-bit virtual address spaces (known as Sv39).
 */
#define VM_MODE (0x8llu << 60)

#define RISCV_PGSHIFT 12

static inline void enable_mmu(void)
{
    // The RISC-V privileged spec (20211203), section 4.1.11 says that the
    // SFENCE.VMA instruction may need to be executed before or after writing
    // to satp. I don't understand why we do it before compared to after.
    // Need to understand 4.2.1 of the spec.
    asm volatile("sfence.vma" ::: "memory");
    asm volatile(
        "csrw satp, %0\n"
        :
        : "r"(VM_MODE | (uintptr_t)boot_lvl1_pt >> RISCV_PGSHIFT)
        :
    );
    asm volatile("fence.i" ::: "memory");
}
#endif

// Multikernel features, powers on extra cpus with their own stack and own kernel entry
#if 1 || defined(NUM_MULTIKERNELS) && NUM_MULTIKERNELS > 1

#define PSCI_SM64_CPU_ON 0xc4000003

// In utils
void disable_caches_el2(void);
void start_secondary_cpu(void);

volatile uint64_t curr_cpu_id;
volatile uintptr_t curr_cpu_stack;
static volatile int core_up[NUM_MULTIKERNELS];

volatile uint64_t cpu_magic;

static inline void dsb(void)
{
    asm volatile("dsb sy" ::: "memory");
}

int psci_func(unsigned long smc_function_id, unsigned long param1, unsigned long param2, unsigned long param3);

int psci_cpu_on(uint64_t cpu_id) {
    dsb();
    curr_cpu_id = cpu_id;
    dsb();
    uintptr_t cpu_stack = (uintptr_t)(&_stack[curr_cpu_id][0xff0]);
    __atomic_store_n(&curr_cpu_stack, cpu_stack, __ATOMIC_SEQ_CST);
    return psci_func(PSCI_SM64_CPU_ON, curr_cpu_id, (unsigned long)&start_secondary_cpu, 0);
}

#define MSR(reg, v)                                \
    do {                                           \
        uint64_t _v = v;                             \
        asm volatile("msr " reg ",%0" :: "r" (_v));\
    } while(0)

void secondary_cpu_entry() {
    dsb();
    uint64_t cpu = curr_cpu_id;

    int r;
    r = ensure_correct_el();
    if (r != 0) {
        goto fail;
    }

    /* Get this CPU's ID and save it to TPIDR_EL1 for seL4. */
    /* Whether or not seL4 is booting in EL2 does not matter, as it always looks at tpidr_el1 */
    MSR("tpidr_el1", cpu);

    uint64_t mpidr_el1;
    asm volatile("mrs %x0, mpidr_el1" : "=r"(mpidr_el1) :: "cc");
    puts("LDR|INFO: secondary (CPU ");
    puthex32(cpu);
    puts(") has MPIDR_EL1: ");
    puthex64(mpidr_el1);
    puts("\n");

    puts("LDR|INFO: enabling MMU (CPU ");
    puthex32(cpu);
    puts(")\n");
    el2_mmu_enable(boot_lvl0_lower[cpu]);

    puts("LDR|INFO: jumping to kernel (CPU ");
    puthex32(cpu);
    puts(")\n");

    dsb();
    __atomic_store_n(&core_up[cpu], 1, __ATOMIC_RELEASE);
    dsb();

#if 1
#ifdef BOARD_odroidc4_multikernel
    for (volatile int i = 0; i < cpu * 10000000; i++);
#elif defined(BOARD_maaxboard_multikernel)
    for (volatile int i = 0; i < cpu * 10000000; i++);
#else
    for (volatile int i = 0; i < cpu * 100000000; i++);
#endif
#endif
    start_kernel(cpu);

    puts("LDR|ERROR: seL4 Loader: Error - KERNEL RETURNED (CPU ");
    puthex32(cpu);
    puts(")\n");

fail:
    /* Note: can't usefully return to U-Boot once we are here. */
    /* IMPROVEMENT: use SMC SVC call to try and power-off / reboot system.
     * or at least go to a WFI loop
     */
    for (;;) {
    }
}

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

void set_exception_handler()
{
#ifdef ARCH_aarch64
    enum el el = current_el();
    if (el == EL2) {
        asm volatile("msr vbar_el2, %0" :: "r"(arm_vector_table));
    }
    /* Since we call the exception handler before we check we're at
     * a valid EL we shouldn't assume we are at EL1 or higher. */
    if (el != EL0) {
        asm volatile("msr vbar_el1, %0" :: "r"(arm_vector_table));
    }
#elif ARCH_riscv64
    /* Don't do anything on RISC-V since we always are in S-mode so M-mode
     * will catch our faults (e.g SBI). */
#else
#error "Unsupported architecture for set_exception_handler"
#endif
}

int main(void)
{
    uart_init();
    /* After any UART initialisation is complete, setup an arch-specific exception
     * handler in case we fault somewhere in the loader. */
    set_exception_handler();

    puts("LDR|INFO: altloader for seL4 starting\n");
    /* Check that the loader magic number is set correctly */
    if (loader_data->magic != MAGIC) {
        puts("LDR|ERROR: mismatch on loader data structure magic number\n");
        goto fail;
    }

    regions = (void *) &(loader_data->kernel_bootinfos_and_regions[loader_data->num_kernels]);

#ifdef ARCH_riscv64
    puts("LDR|INFO: configured with FIRST_HART_ID ");
    puthex32(FIRST_HART_ID);
    puts("\n");
#endif

    print_loader_data();

    /* past here we have trashed u-boot so any errors should go to the
     * fail label; it's not possible to return to U-boot
     */
    copy_data();

#if defined(GIC_VERSION)
#if GIC_VERSION == 2
    puts("LDR|INFO: Initialising interrupt controller GICv2\n");
    configure_gicv2();
#elif GIC_VERSION == 3
    puts("LDR|INFO: Initialising interrupt controller GICv3\n");
    configure_gicv3();
#else
    #error "unknown GIC version"
#endif
#else
    puts("LDR|INFO: No interrupt controller to initialise\n");
#endif

    puts("LDR|INFO: # of multikernels is ");
    putc(num_multikernels + '0');
    puts("\n");

#ifdef ARCH_aarch64
    int r;
    enum el el;
    r = ensure_correct_el();
    if (r != 0) {
        goto fail;
    }

#if 1 || NUM_MULTIKERNELS > 1

    disable_caches_el2();

    /* Get the CPU ID of the CPU we are booting on. */
    uint64_t boot_cpu_id;
    asm volatile("mrs %x0, mpidr_el1" : "=r"(boot_cpu_id) :: "cc");
    boot_cpu_id = boot_cpu_id & 0x00ffffff;
    if (boot_cpu_id >= NUM_MULTIKERNELS) {
        puts("LDR|ERROR: Boot CPU ID (");
        puthex32(boot_cpu_id);
        puts(") exceeds the maximum CPU ID expected (");
        puthex32(NUM_MULTIKERNELS - 1);
        puts(")\n");
        goto fail;
    }
    puts("LDR|INFO: Boot CPU ID (");
    putc(boot_cpu_id + '0');
    puts(")\n");

    /* Start each CPU, other than the one we are booting on. */
    for (int i = 0; i < NUM_MULTIKERNELS; i++) {
        if (i == boot_cpu_id) continue;

        asm volatile("dmb sy" ::: "memory");

        puts("LDR|INFO: Starting other CPUs (");
        puthex32(i);
        puts(")\n");

        r = psci_cpu_on(i);
        /* PSCI success is 0. */
        // TODO: decode PSCI error and print out something meaningful.
        if (r != 0) {
            puts("LDR|ERROR: Failed to start CPU ");
            puthex32(i);
            puts(", PSCI error code is ");
            puthex64(r);
            puts("\n");
            goto fail;
        }

        dsb();
        //while (1); // dont boot 0
        while (!__atomic_load_n(&core_up[i], __ATOMIC_ACQUIRE));
        //for (volatile int i = 0; i < 100000000; i++); // delay boot 0
    }

#endif

    puts("LDR|INFO: enabling self MMU\n");
    el = current_el();
    if (el == EL1) {
        #if 1 || NUM_MULTIKERNELS > 1
        el1_mmu_enable(boot_lvl0_lower[0], boot_lvl0_upper[0]);
        #else
        el1_mmu_enable(boot_lvl0_lower, boot_lvl0_upper);
        #endif
    } else if (el == EL2) {
        #if 1 || NUM_MULTIKERNELS > 1
        el2_mmu_enable(boot_lvl0_lower[0]);
        #else
        el2_mmu_enable(boot_lvl0_lower);
        #endif
    } else {
        puts("LDR|ERROR: unknown EL level for MMU enable\n");
    }
#elif defined(ARCH_riscv64)
    puts("LDR|INFO: enabling MMU\n");
    enable_mmu();
#endif

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
#ifdef ARCH_aarch64
void exception_handler(uintptr_t ex)
{
    /* Read ESR/FSR based on the exception level we're at. */
    uint64_t esr;
    uintptr_t far;

    if (loader_data->flags & FLAG_SEL4_HYP) {
        asm volatile("mrs %0, ESR_EL2" : "=r"(esr) :: "cc");
        asm volatile("mrs %0, FAR_EL2" : "=r"(far) :: "cc");
    } else {
        asm volatile("mrs %0, ESR_EL1" : "=r"(esr) :: "cc");
        asm volatile("mrs %0, FAR_EL1" : "=r"(far) :: "cc");
    }

    uintptr_t ec = (esr >> 26) & 0x3f;
    puts("\nLDR|ERROR: loader trapped exception: ");
    puts(ex_to_string(ex));
    if (loader_data->flags & FLAG_SEL4_HYP) {
        puts("\n    esr_el2: ");
    } else {
        puts("\n    esr_el1: ");
    }
    puthex(esr);
    puts("\n    ec: ");
    puthex32(ec);
    puts(" (");
    puts(ec_to_string(ec));
    puts(")\n    il: ");
    puthex((esr >> 25) & 1);
    puts("\n    iss: ");
    puthex(esr & MASK(24));
    puts("\n    far: ");
    puthex(far);
    puts("\n");

    for (unsigned i = 0; i < 32; i++)  {
        puts("    reg: ");
        puthex32(i);
        puts(": ");
        puthex(exception_register_state[i]);
        puts("\n");
    }

    for (;;) {
    }
}
#endif
