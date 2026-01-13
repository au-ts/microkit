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

ARCH := ${shell grep 'CONFIG_SEL4_ARCH  ' $(BOARD_DIR)/include/kernel/gen_config.h | cut -d' ' -f4}

ifeq ($(ARCH),aarch64)
	TOOLCHAIN := aarch64-none-elf
	# No specific AArch64 flags
	CFLAGS_ARCH :=
else ifeq ($(ARCH),riscv64)
	TOOLCHAIN := riscv64-unknown-elf
	CFLAGS_ARCH := -march=rv64imafdc_zicsr_zifencei -mabi=lp64d
else
$(error Unsupported ARCH)
endif

CC := $(TOOLCHAIN)-gcc
LD := $(TOOLCHAIN)-ld
AS := $(TOOLCHAIN)-as
OBJCOPY := $(TOOLCHAIN)-objcopy
MICROKIT_TOOL ?= $(MICROKIT_SDK)/bin/microkit

DOUBLE_ELF_OBJS := double_elf.o
SECONDARY_OBJS := secondary.o
CONTROLLER_OBJS := controller.o

# compiled but NOT in system description
DYNAMIC_PD_SOURCES := dummy.c
DYNAMIC_PD_OBJS := $(DYNAMIC_PD_SOURCES:.c=.o)
DYNAMIC_PD_ELFS := $(DYNAMIC_PD_SOURCES:.c=.elf)

IMAGES := double_elf.elf secondary.elf controller.elf
CFLAGS := -mstrict-align -nostdlib -ffreestanding -g -O3 -Wall  -Wno-unused-function -Werror -I$(BOARD_DIR)/include $(CFLAGS_ARCH)
LDFLAGS := -L$(BOARD_DIR)/lib
LIBS := -lmicrokit 

IMAGE_FILE = $(BUILD_DIR)/loader.img
REPORT_FILE = $(BUILD_DIR)/report.txt
DYNAMIC_ELFS_BIN = $(BUILD_DIR)/dynamic_elfs.bin

all: $(IMAGE_FILE)

$(BUILD_DIR)/%.o: %.c Makefile
	$(CC) -c $(CFLAGS) $< -o $@

$(BUILD_DIR)/%.o: %.s Makefile
	$(AS) -g -mcpu=$(CPU) $< -o $@

$(BUILD_DIR)/double_elf.elf: $(addprefix $(BUILD_DIR)/, $(DOUBLE_ELF_OBJS))
	$(LD) $(LDFLAGS) $^ $(LIBS) -Tmicrokit.ld -o $@

$(BUILD_DIR)/controller.elf: $(addprefix $(BUILD_DIR)/, $(CONTROLLER_OBJS))
	$(LD) $(LDFLAGS) $^ $(LIBS) -Tmicrokit.ld -o $@

$(BUILD_DIR)/secondary.elf: $(addprefix $(BUILD_DIR)/, $(SECONDARY_OBJS))
	$(LD) $(LDFLAGS) $^ $(LIBS) -Tmicrokit.ld -o $@

$(BUILD_DIR)/%.elf: $(BUILD_DIR)/%.o
	$(LD) $(LDFLAGS) $^ $(LIBS) -Tmicrokit.ld -o $@

$(DYNAMIC_ELFS_BIN): $(addprefix $(BUILD_DIR)/, $(DYNAMIC_PD_ELFS))
	@mkdir -p $(BUILD_DIR)
	@cat $^ > $@

# convert binary blob to object file with linker symbols
$(BUILD_DIR)/dynamic_elfs.o: $(DYNAMIC_ELFS_BIN)
	$(OBJCOPY) -I binary -O elf64-littleaarch64 \
		--rename-section .data=.dynamic_elfs \
		$< $@

$(IMAGE_FILE) $(REPORT_FILE): $(addprefix $(BUILD_DIR)/, $(IMAGES)) double_elf.system $(BUILD_DIR)/dynamic_elfs.o
	$(MICROKIT_TOOL) double_elf.system --search-path $(BUILD_DIR) --board $(MICROKIT_BOARD) --config $(MICROKIT_CONFIG) -o $(IMAGE_FILE) -r $(REPORT_FILE)
	@echo "Dynamic ELFs embedded in monitor"
