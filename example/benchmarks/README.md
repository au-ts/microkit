<!--
     Copyright 2025, UNSW
     SPDX-License-Identifier: CC-BY-SA-4.0
-->
# Example - Benchmarks

This does basic seL4 signalling performance benchmarks.

Note that in these PDs we deliberately subvert the microkit implementation
so that we have direct control over the mechanisms at play.

## signal_low_to_hi_same_core

This is a *one way* benchmark, which relies on the fact that the cycle counter
is a core-local value. This means we can read the cycle count in the low
priority PD and then read it again in the high priority PD and the values will
make sense. We also assume that writes to memory require no cache or memory
barriers due to ARM's coherence provisions.
