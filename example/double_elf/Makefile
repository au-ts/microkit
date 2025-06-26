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
MICROKIT_TOOL ?= $(MICROKIT_SDK)/bin/microkit

DOUBLE_ELF_OBJS := double_elf.o
SECONDARY_OBJS := secondary.o

IMAGES := double_elf.elf secondary.elf
CFLAGS := -mstrict-align -nostdlib -ffreestanding -g -O3 -Wall  -Wno-unused-function -Werror -I$(BOARD_DIR)/include $(CFLAGS_ARCH)
LDFLAGS := -L$(BOARD_DIR)/lib
LIBS := -lmicrokit 

IMAGE_FILE = $(BUILD_DIR)/loader.img
REPORT_FILE = $(BUILD_DIR)/report.txt

all: $(IMAGE_FILE)

$(BUILD_DIR)/%.o: %.c Makefile
	$(CC) -c $(CFLAGS) $< -o $@

$(BUILD_DIR)/%.o: %.s Makefile
	$(AS) -g -mcpu=$(CPU) $< -o $@

$(BUILD_DIR)/double_elf.elf: $(addprefix $(BUILD_DIR)/, $(DOUBLE_ELF_OBJS))
	$(LD) $(LDFLAGS) $^ $(LIBS) -Tmicrokit.ld -o $@

$(BUILD_DIR)/secondary.elf: $(addprefix $(BUILD_DIR)/, $(SECONDARY_OBJS))
	$(LD) $(LDFLAGS) $^ $(LIBS) -Tmicrokit_secondary.ld -o $@

$(IMAGE_FILE) $(REPORT_FILE): $(addprefix $(BUILD_DIR)/, $(IMAGES)) double_elf.system
	$(MICROKIT_TOOL) double_elf.system --search-path $(BUILD_DIR) --board $(MICROKIT_BOARD) --config $(MICROKIT_CONFIG) -o $(IMAGE_FILE) -r $(REPORT_FILE)
