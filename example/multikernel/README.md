<!--
     Copyright 2025, UNSW
     SPDX-License-Identifier: CC-BY-SA-4.0
-->
# Example - Multikernel

```
[first core boots]

core0_A: hello, world (from core 0)
core0_A: notifying same core on 5
core0_B: hello, world (from core 0)
core0_B: notifying same core on 5
core0_A: notified: 5 (same core)
core0_B: notified: 5 (same core)

[second core boots]

core1: hello, world (from core 1)
core1: signalling from core 1 to core 0
core0_A: notified: 0 (cross core)
core0_A: replying from core 0 to core 1
core1: notified: 0 (cross core)
core1: replying from core 1 to core 0
core0_A: notified: 0 (cross core)
core0_A: replying from core 0 to core 1
core1: notified: 0 (cross core)
core1: replying from core 1 to core 0
core0_A: notified: 0 (cross core)
core0_A: replying from core 0 to core 1
core1: notified: 0 (cross core)
core1: replying from core 1 to core 0
core0_A: notified: 0 (cross core)
core0_A: replying from core 0 to core 1
core1: notified: 0 (cross core)
core1: replying from core 1 to core 0
core0_A: notified: 0 (cross core)
core0_A: replying from core 0 to core 1
core1: notified: 0 (cross core)
core1: replying from core 1 to core 0
core0_A: notified: 0 (cross core)
core0_A: replying from core 0 to core 1
core1: notified: 0 (cross core)
core1: stopping after 5 notifications
```
