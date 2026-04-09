#!/usr/bin/env sh

cd /Users/terrybai/ts/microkit/ && \
python build_sdk.py --skip-docs --skip-tar --boards=x86_64_generic,qemu_virt_aarch64 --sel4=~/ts/sel4 --configs=debug,benchmark && \
cd /Users/terrybai/ts/microkit/example/hello && \
make BUILD_DIR=build MICROKIT_BOARD=qemu_virt_aarch64 MICROKIT_CONFIG=benchmark MICROKIT_SDK=/Users/terrybai/ts/microkit/release/microkit-sdk-2.1.0-dev qemu
