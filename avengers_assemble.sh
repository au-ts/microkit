#!/bin/sh

set -eo pipefail

source pyvenv/bin/activate

# pip install -r requirements.txt
# cd tool/microkit && cargo clean && cd ../..
# rm -rfd build tool/target target dep/rust-sel4/target release

python3 build_sdk.py --skip-docs --skip-tar --configs=debug --sel4=/Users/dreamliner787-9/TS/microkit-capdl-dev/seL4 --boards=x86_64_generic,x86_64_generic_vtx

cd example/x86_64_ioport
rm -rfd build
mkdir build
make qemu MICROKIT_SDK=/Users/dreamliner787-9/TS/microkit-capdl-dev/release/microkit-sdk-2.0.1-dev MICROKIT_BOARD=x86_64_generic MICROKIT_CONFIG=debug BUILD_DIR=/Users/dreamliner787-9/TS/microkit-capdl-dev/example/x86_64_ioport/build
