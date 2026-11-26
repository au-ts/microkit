# SPDX-License-Identifier: BSD-2-Clause

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

BOARD_DIR := $(MICROKIT_SDK)/board/$(MICROKIT_BOARD)/$(MICROKIT_CONFIG)
ARCH := $(shell grep 'CONFIG_SEL4_ARCH  ' $(BOARD_DIR)/include/kernel/gen_config.h | cut -d' ' -f4)

ifeq ($(ARCH),riscv64)
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
QEMU ?= qemu-system-riscv64

TEST_OBJS := riscv64_iommu_dma_test.o
IMAGES := riscv64_iommu_dma_test.elf

CFLAGS := -nostdlib -ffreestanding -g -O3 -Wall  -Wno-unused-function -Werror -I$(BOARD_DIR)/include $(CFLAGS_ARCH)
LDFLAGS := -L$(BOARD_DIR)/lib
LIBS := -lmicrokit -Tmicrokit.ld

IMAGE_FILE = $(BUILD_DIR)/loader.img
REPORT_FILE = $(BUILD_DIR)/report.txt
SPEC_FILE = $(BUILD_DIR)/capdl_spec.json

.PHONY: all qemu
all: $(IMAGE_FILE)

$(BUILD_DIR):
	mkdir -p $@

$(BUILD_DIR)/%.o: %.c Makefile | $(BUILD_DIR)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD_DIR)/riscv64_iommu_dma_test.elf: $(addprefix $(BUILD_DIR)/, $(TEST_OBJS))
	$(LD) $(LDFLAGS) $^ $(LIBS) -o $@

$(IMAGE_FILE): $(addprefix $(BUILD_DIR)/, $(IMAGES)) riscv64_iommu_dma_test.system
	$(MICROKIT_TOOL) riscv64_iommu_dma_test.system --search-path $(BUILD_DIR) \
		--board $(MICROKIT_BOARD) --config $(MICROKIT_CONFIG) \
		-o $@ -r $(REPORT_FILE) --capdl-json $(SPEC_FILE)

qemu: $(IMAGE_FILE)
	$(QEMU) -machine virt,aia=aplic-imsic,iommu-sys=on -smp 1 -m 2G \
		-nographic -device edu,dma_mask=0xffffffffffffffff,bus=pcie.0,addr=0x04.0 \
		-kernel $(IMAGE_FILE)
