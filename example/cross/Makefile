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
          -I$(BOARD_DIR)/include/pancake-c-interop $(CFLAGS_ARCH)

LDFLAGS := -L$(BOARD_DIR)/lib

CROSSSTAGE1 := $(BUILD_DIR)/stage1_concat.pnk
CROSSSTAGE2 := $(BUILD_DIR)/stage2_processed.pnk
CROSSSTAGE3 := $(BUILD_DIR)/stage3_assembly.S
CROSSSTAGE4 := $(BUILD_DIR)/stage4_object.o

CROSSELF := $(BUILD_DIR)/cross.elf
IMAGE   := $(BUILD_DIR)/loader.img
REPORT  := $(BUILD_DIR)/report.txt

all: $(IMAGE)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# Stage1: concat the Pancake files together
$(CROSSSTAGE1): cross.pnk | $(BUILD_DIR)
	@cat $(BOARD_DIR)/include/pansel4/pansel4.pnk > $@
	@cat $(BOARD_DIR)/include/panmicrokit/api.pnk >> $@
	@cat $(BOARD_DIR)/include/panmicrokit/main.pnk >> $@
	@cat cross.pnk >> $@

# Stage2: run C preprocessor on Stage1
$(CROSSSTAGE2): $(CROSSSTAGE1) | $(BUILD_DIR)
	$(CPP) -P -I$(BOARD_DIR)/include $< > $@

# Stage3: compile all Pancake code into asm / machine code
$(CROSSSTAGE3): $(CROSSSTAGE2) | $(BUILD_DIR)
	cat $< | cake --target=riscv --pancake --main_return=true > $@

# Stage4: create object file from Stage3 asm
$(CROSSSTAGE4): $(CROSSSTAGE3) | $(BUILD_DIR)
	$(CC) -c -x assembler-with-cpp $(CFLAGS) $< -o $@

# C side
$(BUILD_DIR)/cross.o: cross.c | $(BUILD_DIR)
	$(CC) -c $(CFLAGS) $< -o $@

# Link the final executable against both libraries
$(CROSSELF): $(BUILD_DIR)/cross.o $(CROSSSTAGE4) | $(BUILD_DIR)
	$(LD) $(LDFLAGS) $^ -lmicrokitpnk -T$(BOARD_DIR)/lib/microkit.ld -o $@

# Build the whole system image
$(IMAGE) $(REPORT): $(CROSSELF) cross.system | $(BUILD_DIR)
	$(MICROKIT_TOOL) cross.system --search-path $(BUILD_DIR) \
	  --board $(MICROKIT_BOARD) --config $(MICROKIT_CONFIG) \
	  -o $(IMAGE) -r $(REPORT)

clean:
	rm -rf $(BUILD_DIR)

.PHONY: all clean
