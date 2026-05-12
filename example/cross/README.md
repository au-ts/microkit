# Example - Cross

This is a basic example of a C protection domain running on a Pancake Microkit.

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
