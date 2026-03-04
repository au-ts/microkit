ifeq ($(strip $(BUILD_DIR)),)
$(error BUILD_DIR must be specified)
endif

ifeq ($(strip $(MICROKIT_SDK)),)
$(error MICROKIT_SDK must be specified)
endif

ifeq ($(strip $(MICROKIT_PANCAKE_SDK)),)
$(error MICROKIT_PANCAKE_SDK must be specified. Try the pancake dir in this repo)
endif

ifeq ($(strip $(MICROKIT_BOARD)),)
$(error MICROKIT_BOARD must be specified)
endif

ifeq ($(strip $(MICROKIT_CONFIG)),)
$(error MICROKIT_CONFIG must be specified)
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

CROSSSTAGE1 := $(BUILD_DIR)/stage1_concat.pnk
CROSSSTAGE2 := $(BUILD_DIR)/stage2_processed.pnk
CROSSSTAGE3 := $(BUILD_DIR)/stage3_assembly.S
CROSSSTAGE4 := $(BUILD_DIR)/stage4_object.o

OBJS := \
  $(BUILD_DIR)/crt0.o \
  $(BUILD_DIR)/adjutant.o \
  $(BUILD_DIR)/api.o \
  $(BUILD_DIR)/pansel4.o \
  $(BUILD_DIR)/cross.o \
  $(CROSSSTAGE4)

CROSSELF := $(BUILD_DIR)/cross.elf
IMAGE   := $(BUILD_DIR)/loader.img
REPORT  := $(BUILD_DIR)/report.txt

all: $(IMAGE)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# Stage1: concat the pancake files together
$(CROSSSTAGE1): cross.pnk | $(BUILD_DIR)
	@cat $(MICROKIT_PANCAKE_SDK)/pansel4/pansel4.pnk > $(CROSSSTAGE1)
	@cat $(MICROKIT_PANCAKE_SDK)/panmicrokit/api.pnk >> $(CROSSSTAGE1)
	@cat $(MICROKIT_PANCAKE_SDK)/panmicrokit/handler_loop.pnk >> $(CROSSSTAGE1)
	@cat cross.pnk >> $(CROSSSTAGE1)

# Stage2: run C preprocessor on Stage1
$(CROSSSTAGE2): $(CROSSSTAGE1) | $(BUILD_DIR)
	$(CPP) -P $< > $@

# Stage3: compile all Pancake code into asm / machine code
$(CROSSSTAGE3): $(CROSSSTAGE2) | $(BUILD_DIR)
	cat $< | cake --target=riscv --pancake --main_return=true > $@

# Stage4: create object file from Stage3 asm
$(CROSSSTAGE4): $(CROSSSTAGE3) | $(BUILD_DIR)
	$(CC) -c -x assembler-with-cpp $(CFLAGS) $< -o $@

# panmicrokit library (TODO: move these out of here)

$(BUILD_DIR)/crt0.o:
	$(AS) -c -g $(MICROKIT_PANCAKE_SDK)/panmicrokit/crt0.s -o $@

$(BUILD_DIR)/adjutant.o:
	$(CC) -c $(CFLAGS) $(MICROKIT_PANCAKE_SDK)/panmicrokit/adjutant.c -o $@

$(BUILD_DIR)/api.o:
	$(CC) -c $(CFLAGS) $(MICROKIT_PANCAKE_SDK)/panmicrokit/api.c -o $@

$(BUILD_DIR)/pansel4.o:
	$(CC) -c $(CFLAGS) $(MICROKIT_PANCAKE_SDK)/pansel4/pansel4.c -o $@


# C pd (pong)

$(BUILD_DIR)/cross.o: cross.c | $(BUILD_DIR)
	$(CC) -c $(CFLAGS) cross.c -o $@


# Link everything to create the final Pancake cross executable
$(CROSSELF): $(OBJS) | $(BUILD_DIR)
	$(LD) $(LDFLAGS) $(OBJS) -T$(MICROKIT_PANCAKE_SDK)/panmicrokit/microkit.ld -o $@

# Build the whole system image
$(IMAGE) $(REPORT): $(CROSSELF) cross.system | $(BUILD_DIR)
	$(MICROKIT_TOOL) cross.system --search-path $(BUILD_DIR) \
	  --board $(MICROKIT_BOARD) --config $(MICROKIT_CONFIG) \
	  -o $(IMAGE) -r $(REPORT)

clean:
	rm -rf $(BUILD_DIR)

.PHONY: all clean
