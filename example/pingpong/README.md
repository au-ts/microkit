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
