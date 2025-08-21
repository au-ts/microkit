#!/bin/bash

python3 build_sdk.py --sel4="/home/freya/tor/seL4" --boards odroidc4_multikernel,odroidc4_multikernel_1,odroidc4_multikernel_2  --configs debug \
&& python3 dev_build.py --rebuild --example hello --board odroidc4_multikernel \
&& ~/machine_queue/mq.sh run -s odroidc4_1 -f /home/freya/tor/microkit/tmp_build/loader.img -c "Trying to get this to work" -t 1800
