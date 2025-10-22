#
# Copyright 2025, UNSW.
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

MICROKIT_CONFIG ?= benchmark

BOARD_DIR := $(MICROKIT_SDK)/board/$(MICROKIT_BOARD)/$(MICROKIT_CONFIG)

ARCH := ${shell grep 'CONFIG_SEL4_ARCH  ' $(BOARD_DIR)/include/kernel/gen_config.h | cut -d' ' -f4}

ifeq ($(ARCH),aarch64)
  TARGET_TRIPLE := aarch64-none-elf
  CFLAGS_ARCH := -mstrict-align
else ifeq ($(ARCH),riscv64)
  TARGET_TRIPLE := riscv64-unknown-elf
  CFLAGS_ARCH := -march=rv64imafdc_zicsr_zifencei -mabi=lp64d
else
$(error Unsupported ARCH)
endif

ifeq ($(strip $(LLVM)),True)
  CC := clang -target $(TARGET_TRIPLE)
  AS := clang -target $(TARGET_TRIPLE)
  LD := ld.lld
else
  CC := $(TARGET_TRIPLE)-gcc
  LD := $(TARGET_TRIPLE)-ld
  AS := $(TARGET_TRIPLE)-as
endif

MICROKIT_TOOL ?= $(MICROKIT_SDK)/bin/microkit

IMAGES := manager.elf \
  signal_low_to_hi_same_core__high.elf signal_low_to_hi_same_core__low.elf \
  signal_hi_to_low__high.elf signal_hi_to_low__low.elf

CFLAGS := -MMD -MP -nostdlib -ffreestanding -g -O2 -Wall -Wno-unused-function -I$(BOARD_DIR)/include $(CFLAGS_ARCH)
LDFLAGS := -L$(BOARD_DIR)/lib
LIBS := -lmicrokit -Tmicrokit.ld

IMAGE_FILE = $(BUILD_DIR)/loader.img
REPORT_FILE = $(BUILD_DIR)/report.txt
SYSTEM_FILE = benchmarks.system
SYSTEM_FILE_PP = $(BUILD_DIR)/$(SYSTEM_FILE).pp

all: $(IMAGE_FILE)

$(BUILD_DIR)/%.o: %.c Makefile
	$(CC) -c $(CFLAGS) $< -o $@

$(BUILD_DIR)/%.elf: $(BUILD_DIR)/%.o $(BUILD_DIR)/print.o
	$(LD) $(LDFLAGS) $^ $(LIBS) -o $@

-include $(addprefix $(BUILD_DIR)/, $(IMAGES:.elf=.d))
-include $(BUILD_DIR)/print.d

$(SYSTEM_FILE_PP): $(SYSTEM_FILE)
	cpp $< -o $@
	sed -i '/^#/d' $@
	echo deps $(DEPS)

$(IMAGE_FILE) $(REPORT_FILE): $(addprefix $(BUILD_DIR)/, $(IMAGES)) $(SYSTEM_FILE_PP)
	$(MICROKIT_TOOL) $(SYSTEM_FILE_PP) --search-path $(BUILD_DIR) --board $(MICROKIT_BOARD) --config $(MICROKIT_CONFIG) -o $(IMAGE_FILE) -r $(REPORT_FILE)
