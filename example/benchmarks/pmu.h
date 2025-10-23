#pragma once

/**
 * Architecture specific PMU events.
 * The following should be declared:
 *
 * - `pmu_enable`. This should enable the PMU and set up the cycle
 *    counters, and will ideally reset.
 * - `pmu_read_cycles`. This should read the current cycle counter and any
 *   barriers as required by the architecture.
 * - `CYCLES_MIN`, `CYCLES_MAX`
 * - `cycles_t` type big enough for the cycle counter.
 *
 * Please ensure that functions are marked as `static inline` and check that
 * they are inlined.
 */
#ifdef CONFIG_ARCH_ARM_V8A

/**
 * Architectural reference:
 *  -  'Arm CoreSight Architecture Performance Monitoring Unit Architecture'
 *     ARM IHI 0091 B.a
 *  -  'Arm Architecture Reference Manual'
 *     ARM ARM DDI 0487 L.b
 *  - Cortex-A55 PMU Use-Cases Application Note (with sample code)
 *     Document ID: 107865 (release 1)
 *
 */

#define CYCLES_MAX UINT64_MAX
#define CYCLES_MIN 0

typedef uint64_t cycles_t;

static inline void isb_sy(void) { asm volatile("isb sy" ::: "memory"); }

static inline cycles_t pmccntr_el0(void) {
  cycles_t v;
  /* D24.5.2 in DDI 0487L.b, PMCCNTR_EL0. All 64 bits is CCNT. */
  asm volatile("mrs %0, pmccntr_el0" : "=r"(v) :: "memory");
  /* TODO: From the ARM sample code, I think there's no need for an ISB here.
           But I can't justify this w.r.t the specification...
   */
  return v;
}

/* 3.11 of Use-Cases app note: step 4 */
static inline void pmu_enable(void) {
  uint64_t v;
  asm volatile("mrs %0, pmcr_el0" : "=r"(v));
  v |= (1ull << 0);
  v &= ~(1ull << 3);
  asm volatile("msr pmcr_el0, %0" : : "r"(v));

  asm volatile("mrs %0, pmcntenset_el0" : "=r"(v));
  v |= (1ull << 31);
  asm volatile("msr pmcntenset_el0, %0" : : "r"(v));

#ifdef CONFIG_ARM_HYPERVISOR_SUPPORT
  /* NSH - count cycles in EL2 */
  v = (1ull << 27);
#else
  v = 0;
#endif
  asm volatile("msr pmccfiltr_el0, %0" : : "r"(v));

  /* Zero the cycle counter */
  asm volatile("msr pmccntr_el0, xzr" : :);

  isb_sy();
}

static inline cycles_t pmu_read_cycles(void) { return pmccntr_el0(); }

#else
#error "unsupported architecture"
#endif
