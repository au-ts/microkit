/*
 * Copyright 2021, Breakaway Consulting Pty. Ltd.
 * Copyright 2026, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#pragma once

#if defined(CONFIG_PLAT_ZYNQMP_ZCU102) || defined(CONFIG_PLAT_ZYNQMP_ULTRA96V2)
#define GICD_BASE 0x00F9010000UL
#define GICC_BASE 0x00F9020000UL
#define GIC_VERSION 2
#elif defined(CONFIG_PLAT_QEMU_ARM_VIRT)
#define GICD_BASE 0x8000000UL
#define GICC_BASE 0x8010000UL
#define GIC_VERSION 2
#elif defined(CONFIG_PLAT_ODROIDC4)
#define GICD_BASE 0xffc01000UL
#define GICC_BASE 0xffc02000UL
#define GIC_VERSION 2
#elif defined(CONFIG_PLAT_MAAXBOARD)
/* reg = <0x38800000 0x10000 0x38880000 0xc0000 0x31000000 0x2000 0x31010000 0x2000 0x31020000 0x2000>; */
#define GICD_BASE 0x38800000UL /* size 0x10000 */
#define GIC_VERSION 3
#elif defined(CONFIG_PLAT_TQMA8XQP1GB)
#define GICD_BASE 0x51a00000
#define GIC_VERSION 3
#else
/* #define GIC_VERSION */
#endif

#if defined(GIC_VERSION)

#define SPI_START         32u

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
    for (i = SPI_START; i < nirqs; i += 4) {
        if (is_set(CONFIG_ARM_HYPERVISOR_SUPPORT)) {
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

    for (i = SPI_START; i < nirqs; i += 4) {
        /* IRQs by default target the loader's CPU, assuming it's "0" CPU interface */
        /* This gives core 0 of seL4 "permission" to configure these interrupts */
        /* cannot configure for SGIs/PPIs (irq < 32) */
        gic_dist->ITARGETSRn[i / 4] = TARGET_CPU_ALLINT(target);
    }

    /* level-triggered, 1-N */
    for (i = SPI_START; i < nirqs; i += 32) {
        gic_dist->ICFGRn[i / 32] = 0x55555555;
    }

    /* group 0 for secure; group 1 for non-secure */
    for (i = 0; i < nirqs; i += 32) {
        if (is_set(CONFIG_ARM_HYPERVISOR_SUPPORT) && !is_set(CONFIG_PLAT_QEMU_ARM_VIRT)) {
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

/* Memory map for GIC distributor */
struct gic_dist_map {
    uint32_t ctlr;                /* 0x0000 */
    uint32_t typer;               /* 0x0004 */
    uint32_t iidr;                /* 0x0008 */
    uint32_t res0;                /* 0x000C */
    uint32_t statusr;             /* 0x0010 */
    uint32_t res1[11];            /* [0x0014, 0x0040) */
    uint32_t setspi_nsr;          /* 0x0040 */
    uint32_t res2;                /* 0x0044 */
    uint32_t clrspi_nsr;          /* 0x0048 */
    uint32_t res3;                /* 0x004C */
    uint32_t setspi_sr;           /* 0x0050 */
    uint32_t res4;                /* 0x0054 */
    uint32_t clrspi_sr;           /* 0x0058 */
    uint32_t res5[9];             /* [0x005C, 0x0080) */
    uint32_t igrouprn[32];        /* [0x0080, 0x0100) */

    uint32_t isenablern[32];        /* [0x100, 0x180) */
    uint32_t icenablern[32];        /* [0x180, 0x200) */
    uint32_t ispendrn[32];          /* [0x200, 0x280) */
    uint32_t icpendrn[32];          /* [0x280, 0x300) */
    uint32_t isactivern[32];        /* [0x300, 0x380) */
    uint32_t icactivern[32];        /* [0x380, 0x400) */

    uint32_t ipriorityrn[255];      /* [0x400, 0x7FC) */
    uint32_t res6;                  /* 0x7FC */

    uint32_t itargetsrn[254];       /* [0x800, 0xBF8) */
    uint32_t res7[2];               /* 0xBF8 */

    uint32_t icfgrn[64];            /* [0xC00, 0xD00) */
    uint32_t igrpmodrn[64];         /* [0xD00, 0xE00) */
    uint32_t nsacrn[64];            /* [0xE00, 0xF00) */
    uint32_t sgir;                  /* 0xF00 */
    uint32_t res8[3];               /* [0xF04, 0xF10) */
    uint32_t cpendsgirn[4];         /* [0xF10, 0xF20) */
    uint32_t spendsgirn[4];         /* [0xF20, 0xF30) */
    uint32_t res9[5236];            /* [0x0F30, 0x6100) */

    uint64_t iroutern[960];         /* [0x6100, 0x7F00) irouter<n> to configure IRQs
                                     * with INTID from 32 to 1019. iroutern[0] is the
                                     * interrupt routing for SPI 32 */
};

/* __builtin_offsetof is not in the verification C subset, so we can only check this in
   non-verification builds. We specifically do not declare a macro for the builtin, because
   we do not want break the verification subset by accident. */
_Static_assert(0x6100 == __builtin_offsetof(struct gic_dist_map, iroutern),
               "error_in_gic_dist_map");


#define GICD_CTLR_RWP                (1ULL << 31)
#define GICD_CTLR_ARE_NS             (1ULL << 4)
#define GICD_CTLR_ENABLE_G1NS        (1ULL << 1)
#define GICD_CTLR_ENABLE_G0          (1ULL << 0)

#define GICD_TYPE_LINESNR 0x1f

#define GIC_PRI_IRQ        0xa0

#define IRQ_SET_ALL 0xffffffff

#define MPIDR_AFF0(x) (x & 0xff)
#define MPIDR_AFF1(x) ((x >> 8) & 0xff)
#define MPIDR_AFF2(x) ((x >> 16) & 0xff)
#define MPIDR_AFF3(x) ((x >> 32) & 0xff)

/** Need fro
 * This field tracks writes to:
•GICD_CTLR[2:0], the Group Enables, for transitions from 1 to 0 only.
•GICD_CTLR[7:4], the ARE bits, E1NWF bit and DS bit.
•GICD_ICENABLER<n>
*/
static void gicv3_dist_wait_for_rwp(void)
{
    volatile struct gic_dist_map *gic_dist = (volatile void *)(GICD_BASE);

    while (gic_dist->ctlr & GICD_CTLR_RWP);
}

static inline uint64_t mpidr_to_gic_affinity(void)
{
    uint64_t mpidr;
    asm volatile("mrs %x0, mpidr_el1" : "=r"(mpidr) :: "cc");

    uint64_t affinity = 0;
    affinity = (uint64_t)MPIDR_AFF3(mpidr) << 32 | MPIDR_AFF2(mpidr) << 16 |
               MPIDR_AFF1(mpidr) << 8  | MPIDR_AFF0(mpidr);
    return affinity;
}

static void configure_gicv3(void)
{
    uintptr_t i;
    uint32_t type;
    uint64_t affinity;
    uint32_t priority;
    unsigned int nr_lines;

    volatile struct gic_dist_map *gic_dist = (volatile void *)(GICD_BASE);

    uint32_t ctlr = gic_dist->ctlr;
    const uint32_t ctlr_mask = GICD_CTLR_ARE_NS | GICD_CTLR_ENABLE_G1NS;
    if ((ctlr & ctlr_mask) != ctlr_mask) {
        if (ctlr !=  (ctlr & ~(GICD_CTLR_ENABLE_G1NS))) {
            // printf("GICv3: GICD_CTLR 0x%x -> 0x%lx (Disabling Grp1NS)\n", ctlr, ctlr & ~(GICD_CTLR_ENABLE_G1NS));
            ctlr = ctlr & ~(GICD_CTLR_ENABLE_G1NS);
            gic_dist->ctlr = ctlr;
            gicv3_dist_wait_for_rwp();
        }

        // printf("GICv3: GICD_CTLR 0x%x -> 0x%x (Enabling Grp1NS and ARE_NS)\n", ctlr, ctlr | ctlr_mask);
        gic_dist->ctlr = ctlr | ctlr_mask;
        gicv3_dist_wait_for_rwp();
    }

    // gic_dist->ctlr = 0;
    // gicv3_dist_wait_for_rwp();

    type = gic_dist->typer;
    nr_lines = 32 * ((type & GICD_TYPE_LINESNR) + 1);

    /* Assume level-triggered */
    for (i = SPI_START; i < nr_lines; i += 16) {
        gic_dist->icfgrn[(i / 16)] = 0;
    }

    /* Default priority for global interrupts */
    priority = (GIC_PRI_IRQ << 24 | GIC_PRI_IRQ << 16 | GIC_PRI_IRQ << 8 |
                GIC_PRI_IRQ);
    for (i = SPI_START; i < nr_lines; i += 4) {
        gic_dist->ipriorityrn[(i / 4)] = priority;
    }
    /* Disable and clear all global interrupts */
    for (i = SPI_START; i < nr_lines; i += 32) {
        gic_dist->icenablern[(i / 32)] = IRQ_SET_ALL;
        gic_dist->icpendrn[(i / 32)] = IRQ_SET_ALL;
    }

    /* Turn on the distributor */
    // gic_dist->ctlr = GICD_CTLR_ARE_NS | GICD_CTLR_ENABLE_G1NS;

    /* Route all global IRQs to this CPU (CPU 0) */
    affinity = mpidr_to_gic_affinity();
    for (i = SPI_START; i < nr_lines; i++) {
        gic_dist->iroutern[i - SPI_START] = affinity;
    }
}

#else
#error "unknown GIC version"
#endif

/* not a GIC */

#endif