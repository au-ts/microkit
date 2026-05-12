<!--
     Copyright 2026, UNSW
     SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Example - Ping pong

This is a basic example with two protection domains that constantly notify
each other.

One of them (`ping.pnk`) is pure Pancake, running on a Pancake libmicrokit,
the other (`pong.c`) is pure C, running on a C microkit.

## Building

```sh
mkdir build
make \
  ARCH=riscv64 \
  BUILD_DIR=./build \
  MICROKIT_SDK=path/to/sdk \
  MICROKIT_BOARD=qemu_virt_riscv64 \
  MICROKIT_CONFIG=debug
```

## Running

```sh
qemu-system-riscv64 \
  -machine virt \
  -nographic \
  -serial mon:stdio \
  -m size=2G \
  -kernel build/loader.img
```

Tested only on RISC-V qemu.

## Verifying

To verify the `ping` protection domain against the specification, you have to
transpile it to Viper code first. This has the `pancake2viper` transpiler as an
additional dependency. You will need a recent version with support for
`/@ extern function ... @/` annotations.

After obtaining `pancake2viper`, you will be able to use the `verify` target
by invoking
```sh
mkdir build
make \
  ARCH=riscv64 \
  BUILD_DIR=./build \
  MICROKIT_SDK=~/microkit/panmicrokit-sdk/ \
  MICROKIT_BOARD=qemu_virt_riscv64 \
  MICROKIT_CONFIG=debug \
  verify
```
and then inspect `build/ping_verification.vpr` with Viper / VS Code to complete
the verification.
