#!/usr/bin/env sh

cd /Users/terrybai/ts/microkit/ && \
python build_sdk.py --skip-docs --skip-tar --boards=qemu_virt_aarch64 --sel4=~/ts/sel4 --configs=debug,benchmark && \
cd /Users/terrybai/ts/microkit/example/timer && \
make BUILD_DIR=build MICROKIT_BOARD=qemu_virt_aarch64 MICROKIT_CONFIG=debug MICROKIT_SDK=/Users/terrybai/ts/microkit/release/microkit-sdk-2.1.0-dev qemu
