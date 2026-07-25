<!--
     Copyright 2026, UNSW
     SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Example - libmicrokit proof

This example generates a Viper proof showing that, given any valid SDF
configuration, the Pancake version of `libmicrokit` behaves as follows:

1. The `handler_loop` never terminates.
2. Each iteration of the `handler_loop` performs exactly one receive operation:
   an ordinary receive when no response or deferred signal is pending; a
   reply-and-receive when the previous event requires a response; a
   send-and-receive operation when Microkit has queued a deferred notification.
3. Every response returned by the receive operation is handled
   properly: a protected procedure call on channel `c` with message `m` causes
   exactly one call to `uep_protected(c, m)` user entry point; a fault from
   child `c` with message `m` causes exactly one call to `uep_fault(c, m)`; and
   a notification badge causes exactly one call to `uep_notified(c)` for each
   channel `c` whose bit appears in the badge.
4. No *phantom calls* occur: none of the user entry points are ever invoked for
   an event that was not received.
5. When an incoming protected call requires a response, the loop records that
   obligation and sends the returned reply before performing another ordinary
   receive (and similarly for faults).

This is a strengthening of the `Property <>` previously proved using Gordian
for the C implementation.

This proof further shows that all the implemented Microkit API functions obey
their specifications.

**What is assumed**

The current proof has the following trust base:

* The Viper verifier itself (and its dependencies such as SMT solvers),
* the Pancake-to-Viper translator and its Pancake semantics,
* the axiomatization of the SDF-CapDL correspondence in `caps.vpr`,
* the C compiler and the code in `src/panmicrokit/adjutant.c`,
* the assumptions the Pancake libsel4 wrapper makes about the C libsel4 implementation,
* the bitvector, heap, and `IArray` memory axioms used in Viper.

The proof assumes that some external initializer started the PD in a state
where it has no pending reply obligations, and the patched IRQ badge correctly
represents configured IRQ channels. We do not show that the boot code, CapDL
initializer, or the Microkit tool's blitted constants actually create such an
initial state.

The proof does not show that arbitrary user entry point code will use the
`deferred_notify` mechanism correctly: it is possible to misuse this mechanism
in various ways (e.g. by overwriting `have_signal` to abort a notification).
Correct use of `deferred_notify` has to be shown during the concrete PD proofs
(see `example/ivpump`). The most recent version of the C `libmicrokit` (2.3.0)
calls `deferred_flush()` before a `ReplyRecv` to prevent such misuse, but the
verified version is based on version 2.1.0, which does not.

## Building

Dependencies:

* a working installation of the Pancake Microkit SDK (from this repo),
* the `pancake2viper` transpiler, built from commit `4badf62ead` ([repo](https://github.com/au-ts/pancake-transpiler-private/)), and
* the `riscv64-unknown-elf-` compiler toolchain ([repo](https://github.com/riscv-collab/riscv-gnu-toolchain)).

To build the verification file, run

```sh
mkdir build
make \
  ARCH=riscv64 \
  BUILD_DIR=./build \
  MICROKIT_SDK=path/to/sdk \
  MICROKIT_BOARD=qemu_virt_riscv64 \
  MICROKIT_CONFIG=debug \
  verify
```

## Verifying

Checking the proof requires a working installation of VS Code with the Viper extension ([site](https://marketplace.visualstudio.com/items?itemName=viper-admin.viper)).

The verification file is created during the build, and can be loaded using
```sh
code build/panmk_verification.vpr
```
The bottom info area will briefly show the message `Hello from Viper`, then
transition to a progress bar
```
Verification of panmk_verification.vpr
```
signifying that verification has started. Verification should take between 7
and 20 seconds depending on your computer's performance. Eventually, the
message will change to
```
Verified panmk_verification.vpr with 150 warnings
```
indicating successful proof of handler loop correctness.
