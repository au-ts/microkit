#!/usr/bin/env sh

cd ../../ && \
python build_sdk.py --boards=x86_64_generic --configs=debug --sel4=~/ts/seL4 --skip-docs --skip-tar && \
cd example/x86_64_ioport && \
rm -rf ./build && \
mkdir build && \
make BUILD_DIR=build MICROKIT_BOARD=x86_64_generic MICROKIT_CONFIG=debug MICROKIT_SDK=/Users/terrybai/ts/microkit/release/microkit-sdk-2.1.0-dev/ X86_BOARD=qemu_virt_x86 && \
scp ./build/sel4.elf tftp://tftpboot/viscous/sel4kernel && \
scp ./build/initialiser.elf tftp://tftpboot/viscous/sel4rootserver
