<!--
     Copyright 2025, UNSW
     SPDX-License-Identifier: CC-BY-SA-4.0
-->
# Example - Multikernel Memory

```
[[ core 0 starts ]]
core0: hello, world (from core 0)
core0: shared_v: 50331648
core0: shared_p: 4118802432
core0: shared value: 0
[[ core 1 starts ]]

core1: hello, world (from core 1)
core1: shared_v: 50331648
core1: shared_p: 4118802432
core1: shared value: 0
core1: new shared value: 128
core0: notified: 0 (cross core)
core0: shared value: 128
```
