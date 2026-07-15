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
SYSTEM := ivpump.system

PUMP_STAGE1 := $(BUILD_DIR)/pump_stage1_concat.pnk
PUMP_STAGE2 := $(BUILD_DIR)/pump_stage2_processed.pnk
PUMP_STAGE3 := $(BUILD_DIR)/pump_stage3_assembly.S
PUMP_STAGE4 := $(BUILD_DIR)/pump_stage4_object.o
PUMP_ELF := $(BUILD_DIR)/pump.elf
PUMP_CAPS := $(BUILD_DIR)/caps-for-pump.vpr
PUMP_VIPER := $(BUILD_DIR)/pump_verification.vpr

KEYPAD_STAGE1 := $(BUILD_DIR)/keypad_stage1_concat.pnk
KEYPAD_STAGE2 := $(BUILD_DIR)/keypad_stage2_processed.pnk
KEYPAD_STAGE3 := $(BUILD_DIR)/keypad_stage3_assembly.S
KEYPAD_STAGE4 := $(BUILD_DIR)/keypad_stage4_object.o
KEYPAD_ELF := $(BUILD_DIR)/keypad.elf
KEYPAD_CAPS := $(BUILD_DIR)/caps-for-keypad.vpr
KEYPAD_VIPER := $(BUILD_DIR)/keypad_verification.vpr

IMAGE := $(BUILD_DIR)/loader.img
REPORT := $(BUILD_DIR)/report.txt

all: $(IMAGE)


# Pump Pancake pipeline

$(PUMP_STAGE1): pump.pnk
	mkdir -p $(BUILD_DIR)
	cat $(BOARD_DIR)/include/pansel4/pansel4.pnk > $@
	cat $(BOARD_DIR)/include/panmicrokit/api.pnk >> $@
	cat $(BOARD_DIR)/include/panmicrokit/main.pnk >> $@
	cat pump.pnk >> $@

$(PUMP_STAGE2): $(PUMP_STAGE1)
	mkdir -p $(BUILD_DIR)
	$(CPP) -P -I$(BOARD_DIR)/include $< > $@

$(PUMP_STAGE3): $(PUMP_STAGE2)
	mkdir -p $(BUILD_DIR)
	cat $< | cake --target=riscv --pancake --main_return=true > $@

$(PUMP_STAGE4): $(PUMP_STAGE3)
	mkdir -p $(BUILD_DIR)
	$(CC) -c -x assembler-with-cpp $(CFLAGS) $< -o $@

$(PUMP_ELF): $(PUMP_STAGE4)
	mkdir -p $(BUILD_DIR)
	$(LD) $(LDFLAGS) $< -lmicrokitpnk -T$(MICROKIT_LD) -o $@


# Keypad Pancake pipeline

$(KEYPAD_STAGE1): keypad.pnk
	mkdir -p $(BUILD_DIR)
	cat $(BOARD_DIR)/include/pansel4/pansel4.pnk > $@
	cat $(BOARD_DIR)/include/panmicrokit/api.pnk >> $@
	cat $(BOARD_DIR)/include/panmicrokit/main.pnk >> $@
	cat keypad.pnk >> $@

$(KEYPAD_STAGE2): $(KEYPAD_STAGE1)
	mkdir -p $(BUILD_DIR)
	$(CPP) -P -I$(BOARD_DIR)/include $< > $@

$(KEYPAD_STAGE3): $(KEYPAD_STAGE2)
	mkdir -p $(BUILD_DIR)
	cat $< | cake --target=riscv --pancake --main_return=true > $@

$(KEYPAD_STAGE4): $(KEYPAD_STAGE3)
	mkdir -p $(BUILD_DIR)
	$(CC) -c -x assembler-with-cpp $(CFLAGS) $< -o $@

$(KEYPAD_ELF): $(KEYPAD_STAGE4)
	mkdir -p $(BUILD_DIR)
	$(LD) $(LDFLAGS) $< -lmicrokitpnk -T$(MICROKIT_LD) -o $@


# Build the complete system image and generate capability specifications.

$(IMAGE): $(PUMP_ELF) $(KEYPAD_ELF) $(SYSTEM)
	mkdir -p $(BUILD_DIR)
	$(MICROKIT_TOOL) $(SYSTEM) \
		--search-path $(BUILD_DIR) \
		--board $(MICROKIT_BOARD) \
		--config $(MICROKIT_CONFIG) \
		--viper-output $(BUILD_DIR)/caps-for- \
		-o $(IMAGE) \
		-r $(REPORT)


# Pump verification pipeline

verify-pump: $(IMAGE) $(PUMP_STAGE2)
	pancake2viper transpile \
		--disable-prelude \
                --allow-undefined-shared \
		-I $(BOARD_DIR)/include/prelude.vpr \
		-I $(BOARD_DIR)/include/pansel4/spec.vpr \
		-I $(BOARD_DIR)/include/panmicrokit/spec.vpr \
		-I $(PUMP_CAPS) \
		-I ./spec-for-pump.vpr \
		$(PUMP_STAGE2) \
		$(PUMP_VIPER)

	sed -i 's/  patched/  gv.patched/g' $(PUMP_VIPER)
	sed -i 's/ microkit_signal_msg :/ gv.microkit_signal_msg :/g' $(PUMP_VIPER)

	sed -i 's/label continue/\/\/label continue/g' $(PUMP_VIPER)
	sed -i 's/label break/\/\/label break/g' $(PUMP_VIPER)

	sed -i 's/refute false/\/\/refute false/g' $(PUMP_VIPER)


# Keypad verification pipeline

verify-keypad: $(IMAGE) $(KEYPAD_STAGE2)
	pancake2viper transpile \
		--disable-prelude \
                --allow-undefined-shared \
		-I $(BOARD_DIR)/include/prelude.vpr \
		-I $(BOARD_DIR)/include/pansel4/spec.vpr \
		-I $(BOARD_DIR)/include/panmicrokit/spec.vpr \
		-I $(KEYPAD_CAPS) \
		-I ./spec-for-keypad.vpr \
		$(KEYPAD_STAGE2) \
		$(KEYPAD_VIPER)

	sed -i 's/  patched/  gv.patched/g' $(KEYPAD_VIPER)
	sed -i 's/ microkit_signal_msg :/ gv.microkit_signal_msg :/g' $(KEYPAD_VIPER)

	sed -i 's/label continue/\/\/label continue/g' $(KEYPAD_VIPER)
	sed -i 's/label break/\/\/label break/g' $(KEYPAD_VIPER)

	sed -i 's/refute false/\/\/refute false/g' $(KEYPAD_VIPER)

verify: verify-pump verify-keypad

clean:
	rm -rf $(BUILD_DIR)

.PHONY: all clean verify verify-pump verify-keypad
