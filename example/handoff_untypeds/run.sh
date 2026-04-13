#!/usr/bin/env sh

cd /Users/terrybai/ts/microkit/ && \
python build_sdk.py --skip-docs --skip-tar --boards=x86_64_generic --sel4=~/ts/sel4 --configs=debug && \
cd /Users/terrybai/ts/microkit/example/handoff_untypeds && \
make BUILD_DIR=build MICROKIT_BOARD=x86_64_generic MICROKIT_CONFIG=debug MICROKIT_SDK=/Users/terrybai/ts/microkit/release/microkit-sdk-2.1.0-dev qemu
