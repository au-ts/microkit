<!--
     Copyright 2025, UNSW
     SPDX-License-Identifier: CC-BY-SA-4.0
-->
# Example - Benchmarks

This does basic seL4 signalling performance benchmarks.

Note that in these PDs we deliberately subvert the microkit implementation
so that we have direct control over the mechanisms at play.

## signal_low_to_hi_same_core

This is a one way benchmark, which relies on the fact that the cycle counter
is a core-local value. This means we can read the cycle count in the low
priority PD and then read it again in the high priority PD and the values will
make sense.

This measures the time from a seL4_Signal in a low priority process to the
seL4_Recv in the higher priority process, i.e. both send and receive. The
cycle counters are measured in different threads and the end time communicated
back via shared memory. This is because the next run after the signaller is the
*destination*, not the *sender*.

## signal_hi_to_low_same_core

This measures the time from a seL4_Signal in a high priority process to when
that signal returns. This is because higher priority processes will always run
above low priority, so the next running will be the *sender*. This is **different**
to the case for low to high.
