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

STAGE1 := $(BUILD_DIR)/stage1_concat.pnk
STAGE2 := $(BUILD_DIR)/stage2_processed.pnk
STAGE3  := $(BUILD_DIR)/stage3_assembly.S
STAGE4  := $(BUILD_DIR)/stage4_object.o

OBJS := \
  $(BUILD_DIR)/crt0.o \
  $(BUILD_DIR)/adjutant.o \
  $(BUILD_DIR)/api.o \
  $(BUILD_DIR)/pansel4.o \
  $(STAGE4)

ELF    := $(BUILD_DIR)/hello.elf
IMAGE  := $(BUILD_DIR)/loader.img
REPORT := $(BUILD_DIR)/report.txt

all: $(IMAGE)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

# Stage1: concat the pancake files together
$(STAGE1): pansel4/pansel4.pnk panmicrokit/api.pnk panmicrokit/handler_loop.pnk hello.pnk | $(BUILD_DIR)
	@cat pansel4/pansel4.pnk > $(STAGE1)
	@cat panmicrokit/api.pnk >> $(STAGE1)
	@cat panmicrokit/handler_loop.pnk >> $(STAGE1)
	@cat hello.pnk >> $(STAGE1)

# Stage2: run C preprocessor on Stage1
$(STAGE2): $(STAGE1) | $(BUILD_DIR)
	$(CPP) -P $< > $@

# Stage3: compile all Pancake code into asm / machine code
$(STAGE3): $(STAGE2) | $(BUILD_DIR)
	cat $< | cake --target=riscv --pancake --main_return=true > $@

# Stage4: create object file from Stage3 asm
$(STAGE4): $(STAGE3) | $(BUILD_DIR)
	$(CC) -c -x assembler-with-cpp $(CFLAGS) $< -o $@

# C + asm objects
$(BUILD_DIR)/crt0.o: panmicrokit/crt0.s | $(BUILD_DIR)
	$(AS) -c -g $< -o $@

$(BUILD_DIR)/adjutant.o: panmicrokit/adjutant.c | $(BUILD_DIR)
	$(CC) -c $(CFLAGS) $< -o $@

$(BUILD_DIR)/api.o: panmicrokit/api.c | $(BUILD_DIR)
	$(CC) -c $(CFLAGS) $< -o $@

$(BUILD_DIR)/pansel4.o: pansel4/pansel4.c | $(BUILD_DIR)
	$(CC) -c $(CFLAGS) $< -o $@

# Link everything to create the final PD executable
$(ELF): $(OBJS) panmicrokit/microkit.ld | $(BUILD_DIR)
	$(LD) $(LDFLAGS) $(OBJS) -Tpanmicrokit/microkit.ld -o $@

# Build the whole system image
$(IMAGE) $(REPORT): $(ELF) hello.system | $(BUILD_DIR)
	$(MICROKIT_TOOL) hello.system --search-path $(BUILD_DIR) \
	  --board $(MICROKIT_BOARD) --config $(MICROKIT_CONFIG) \
	  -o $(IMAGE) -r $(REPORT)

clean:
	rm -rf $(BUILD_DIR)

.PHONY: all clean
