#
# Copyright 2026, UNSW
#
# SPDX-License-Identifier: BSD-2-Clause
#

ifeq ($(strip $(BUILD_DIR)),)
$(error BUILD_DIR must be specified)
endif

ifeq ($(strip $(MICROKIT_SDK)),)
$(error MICROKIT_SDK must be specified)
endif

ifeq ($(strip $(MICROKIT_BOARD)),)
$(error MICROKIT_BOARD must be specified)
endif

ifeq ($(strip $(MICROKIT_CONFIG)),)
$(error MICROKIT_CONFIG must be specified)
endif

ifeq ($(strip $(ARCH)),)
$(error ARCH must be specified)
endif

BOARD_DIR := $(MICROKIT_SDK)/board/$(MICROKIT_BOARD)/$(MICROKIT_CONFIG)

ifeq ($(ARCH),riscv64)
  TARGET_TRIPLE := riscv64-unknown-elf
  CFLAGS_ARCH   := -mcmodel=medany -march=rv64imafdc_zicsr_zifencei -mabi=lp64d
else
$(error ARCH must be riscv64)
endif

ifeq ($(strip $(LLVM)),True)
  CC  := clang -target $(TARGET_TRIPLE)
  CPP := clang-cpp -target $(TARGET_TRIPLE)
  AS  := clang -target $(TARGET_TRIPLE)
  LD  := ld.lld
else
  CC  := $(TARGET_TRIPLE)-gcc
  CPP := $(TARGET_TRIPLE)-cpp
  AS  := $(TARGET_TRIPLE)-as
  LD  := $(TARGET_TRIPLE)-ld
endif

MICROKIT_TOOL ?= $(MICROKIT_SDK)/bin/microkit

CFLAGS := -nostdlib -ffreestanding -g -O3 -Wall -Werror -Wno-unused-function \
          -I$(BOARD_DIR)/include $(CFLAGS_ARCH)

LDFLAGS := -L$(BOARD_DIR)/lib

PINGSTAGE1 := $(BUILD_DIR)/stage1_concat.pnk
PINGSTAGE2 := $(BUILD_DIR)/stage2_processed.pnk
PINGSTAGE3 := $(BUILD_DIR)/stage3_assembly.S
PINGSTAGE4 := $(BUILD_DIR)/stage4_object.o


PINGELF   := $(BUILD_DIR)/ping.elf
PONGELF   := $(BUILD_DIR)/pong.elf
IMAGE     := $(BUILD_DIR)/loader.img
REPORT    := $(BUILD_DIR)/report.txt
VIPER_OUT := $(BUILD_DIR)/ping_verification.vpr

all: $(IMAGE)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# Stage1: concat the Pancake files together
$(PINGSTAGE1): ping.pnk | $(BUILD_DIR)
	@cat $(BOARD_DIR)/include/pansel4/pansel4.pnk > $@
	@cat $(BOARD_DIR)/include/panmicrokit/api.pnk >> $@
	@cat $(BOARD_DIR)/include/panmicrokit/main.pnk >> $@
	@cat ping.pnk >> $@

# Stage2: run C preprocessor on Stage1
$(PINGSTAGE2): $(PINGSTAGE1) | $(BUILD_DIR)
	$(CPP) -P -I$(BOARD_DIR)/include $< > $@

# Stage3: compile all Pancake code into asm / machine code
$(PINGSTAGE3): $(PINGSTAGE2) | $(BUILD_DIR)
	cat $< | cake --target=riscv --pancake --main_return=true > $@

# Stage4: create object file from Stage3 asm
$(PINGSTAGE4): $(PINGSTAGE3) | $(BUILD_DIR)
	$(CC) -c -x assembler-with-cpp $(CFLAGS) $< -o $@


# C pd (pong)
$(BUILD_DIR)/pong.o: pong.c | $(BUILD_DIR)
	$(CC) -c $(CFLAGS) $< -o $@

MICROKIT_LD := $(BOARD_DIR)/lib/microkit.ld
# Link the final Pancake ping executable against libmicrokitpnk
$(PINGELF): $(PINGSTAGE4) | $(BUILD_DIR)
	$(LD) $(LDFLAGS) $< -lmicrokitpnk -T$(MICROKIT_LD) -o $@

# Link the final C pong executable against libmicrokit
$(PONGELF): $(BUILD_DIR)/pong.o | $(BUILD_DIR)
	$(LD) $(LDFLAGS) $< -lmicrokit -T$(BOARD_DIR)/lib/microkit.ld -o $@

# Build the whole system image
$(IMAGE) $(REPORT): $(PINGELF) $(PONGELF) pingpong.system | $(BUILD_DIR)
	$(MICROKIT_TOOL) pingpong.system \
	  --search-path $(BUILD_DIR) \
	  --board $(MICROKIT_BOARD) \
	  --config $(MICROKIT_CONFIG) \
	  --viper-output build/caps-for- \
	  -o $(IMAGE) -r $(REPORT)

# Output verifiable Viper
verify: $(IMAGE) $(REPORT) $(PINGSTAGE2)
	pancake2viper transpile \
	  --disable-prelude \
	  -I $(BOARD_DIR)/include/prelude.vpr \
	  -I $(BOARD_DIR)/include/pansel4/spec.vpr \
	  -I $(BOARD_DIR)/include/panmicrokit/spec.vpr \
	  -I $(BUILD_DIR)/caps-for-ping.vpr \
	  -I ./spec-for-ping.vpr \
	  $(PINGSTAGE2) \
	  $(VIPER_OUT)

	# FIXME: Workaround for transpiler issue #6
	sed -i 's/  patched/  gv.patched/g' $(VIPER_OUT)
	sed -i 's/ microkit_signal_msg :/ gv.microkit_signal_msg :/g' $(VIPER_OUT)

	# FIXME: Workaround for transpiler issue #7
	sed -i 's/label continue/\/\/label continue/g' $(VIPER_OUT)
	sed -i 's/label break/\/\/label break/g' $(VIPER_OUT)

	# Disable refute false checks
        # Refute false is a Viper idiom to prevent inconsistent sets of assumptions.
        # We _allow_ inconsistent assumption sets, for example calling
        # microkit_ppcall(..) should be impossible if there are no ppcalls defined in
        # the system description file.
	sed -i 's/refute false/\/\/refute false/g' $(VIPER_OUT)

clean:
	rm -rf $(BUILD_DIR)

.PHONY: all clean verify
