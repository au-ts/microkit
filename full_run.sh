#!/bin/bash

python3 build_sdk.py --sel4="/home/freya/tor/seL4" --boards odroidc4_multikernel --configs debug \
&& python3 dev_build.py --rebuild --example hello --board odroidc4_multikernel \
&& cd ~/machine_queue \
&& ./mq.sh run -s odroidc4_pool -f /home/freya/tor/microkit/tmp_build/loader.img -c "Trying to get this to work"
