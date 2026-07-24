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
CFLAGS_ARCH := -mcmodel=medany -march=rv64imafdc_zicsr_zifencei -mabi=lp64d
else
$(error ARCH must be riscv64)
endif

ifeq ($(strip $(LLVM)),True)
CC := clang -target $(TARGET_TRIPLE)
CPP := clang-cpp -target $(TARGET_TRIPLE)
LD := ld.lld
else
CC := $(TARGET_TRIPLE)-gcc
CPP := $(TARGET_TRIPLE)-cpp
LD := $(TARGET_TRIPLE)-ld
endif

MICROKIT_TOOL ?= $(MICROKIT_SDK)/bin/microkit

CFLAGS := -nostdlib -ffreestanding -g -O3 -Wall -Werror \
	-Wno-unused-function \
	-I$(BOARD_DIR)/include \
	$(CFLAGS_ARCH)

LDFLAGS := -L$(BOARD_DIR)/lib

MICROKIT_LD := $(BOARD_DIR)/lib/microkit.ld

PANMK_STAGE1 := $(BUILD_DIR)/panmk_stage1_concat.pnk
PANMK_STAGE2 := $(BUILD_DIR)/panmk_stage2_processed.pnk
PANMK_VIPER := $(BUILD_DIR)/panmk_verification.vpr

# Pancake compilation pipeline

$(PANMK_STAGE1): panmk.pnk
	mkdir -p $(BUILD_DIR)
	cat $(BOARD_DIR)/include/pansel4/pansel4.pnk > $@
	cat $(BOARD_DIR)/include/panmicrokit/api.pnk >> $@
	cat $(BOARD_DIR)/include/panmicrokit/main.pnk >> $@
	cat panmk.pnk >> $@

$(PANMK_STAGE2): $(PANMK_STAGE1)
	mkdir -p $(BUILD_DIR)
	$(CPP) -P -I$(BOARD_DIR)/include $< > $@

verify: $(PANMK_STAGE2)
	pancake2viper transpile \
		--disable-prelude \
                --allow-undefined-shared \
		-I $(BOARD_DIR)/include/prelude.vpr \
		-I ./caps.vpr \
		-I $(BOARD_DIR)/include/pansel4/spec.vpr \
		-I $(BOARD_DIR)/include/panmicrokit/spec.vpr \
		-I ./spec-for-panmk.vpr \
		$(PANMK_STAGE2) \
		$(PANMK_VIPER)

	sed -i 's/  patched/  gv.patched/g' $(PANMK_VIPER)
	sed -i 's/ microkit_signal_msg :/ gv.microkit_signal_msg :/g' $(PANMK_VIPER)

	sed -i 's/label continue/\/\/label continue/g' $(PANMK_VIPER)
	sed -i 's/label break/\/\/label break/g' $(PANMK_VIPER)

	sed -i 's/refute false/\/\/refute false/g' $(PANMK_VIPER)


clean:
	rm -rf $(BUILD_DIR)

.PHONY: clean verify
