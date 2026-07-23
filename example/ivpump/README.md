<!--
     Copyright 2026, UNSW
     SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Example - IV Pump

This is a basic example with two Pancake protection domains and local state
machine models.

## Building

Dependencies:

* a working installation of the Pancake Microkit SDK (from this repo),
* the CakeML compiler, version 16 Jan 2026 or newer ([repo](https://github.com/CakeML/cakeml/tree/master)),
* the `pancake2viper` transpiler, built from commit `4badf62ead` ([repo](https://github.com/au-ts/pancake-transpiler-private/)), and
* the `riscv64-unknown-elf-` compiler toolchain ([repo](https://github.com/riscv-collab/riscv-gnu-toolchain)).

To build the system image and verification files, run

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

## Running

You will need an installation of `qemu` to run the actual PDs. After
building the system image, you can run it using

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

Proof dependencies:

* VS Code with the Viper extension ([site](https://marketplace.visualstudio.com/items?itemName=viper-admin.viper)),
* Agda version 2.8.0 or newer ([site](https://agda.readthedocs.io/en/latest/getting-started/what-is-agda.html)), and
* an installed, compatible version of the Agda standard library ([repo](https://github.com/agda/agda-stdlib)).

The verification files are created during the build, and are located under

* `build/keypad_verification.vpr`,
* `build/pump_verification.vpr`,
* `global/Theorem.agda`.

See `VERIFICATION.md` for details.
