#!/bin/bash

pushd /root/dev-microkit/primary/example/verification/

rm -rf build/
mkdir build

make \
  ARCH=riscv64 \
  BUILD_DIR=./build \
  MICROKIT_SDK=~/microkit/panmicrokit-sdk/ \
  MICROKIT_BOARD=qemu_virt_riscv64 \
  MICROKIT_CONFIG=debug \
  verify

popd
