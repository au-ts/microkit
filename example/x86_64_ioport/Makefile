#
# Copyright 2025, UNSW
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

BOARD_DIR := $(MICROKIT_SDK)/board/$(MICROKIT_BOARD)/$(MICROKIT_CONFIG)

ARCH := ${shell grep 'CONFIG_SEL4_ARCH  ' $(BOARD_DIR)/include/kernel/gen_config.h | cut -d' ' -f4}

ifeq ($(ARCH),x86_64)
	TOOLCHAIN := x86_64-linux-gnu
	CFLAGS_ARCH := -march=x86-64 -mtune=generic
else
$(error Unsupported ARCH)
endif

CC := $(TOOLCHAIN)-gcc
LD := $(TOOLCHAIN)-ld
AS := $(TOOLCHAIN)-as
OBJCOPY := $(TOOLCHAIN)-objcopy
MICROKIT_TOOL ?= $(MICROKIT_SDK)/bin/microkit

IMAGES := server.elf client.elf hpet_timer.elf stack_overflow.elf
# We don't turn on compiler optimisation so that the crashing code doesn't get optimised out.
CFLAGS := -nostdlib -ffreestanding -g3 -I$(BOARD_DIR)/include $(CFLAGS_ARCH)
LDFLAGS := -L$(BOARD_DIR)/lib -z noexecstack
LIBS := -lmicrokit -Tmicrokit.ld

INITIAL_TASK = $(BUILD_DIR)/capdl_initialiser_with_spec.elf
KERNEL = $(MICROKIT_SDK)/board/$(MICROKIT_BOARD)/$(MICROKIT_CONFIG)/elf/sel4.elf
COPIED_KERNEL = $(BUILD_DIR)/kernel.elf
REPORT = $(BUILD_DIR)/report.json

all qemu: $(COPIED_KERNEL) $(INITIAL_TASK)

$(BUILD_DIR)/client.o: client.c Makefile
	$(CC) -c $(CFLAGS) -Wall -Wno-unused-function -Werror $< -o $@
$(BUILD_DIR)/server.o: server.c Makefile
	$(CC) -c $(CFLAGS) -Wall -Wno-unused-function -Werror $< -o $@
$(BUILD_DIR)/hpet_timer.o: hpet_timer.c Makefile
	$(CC) -c $(CFLAGS) -Wall -Wno-unused-function -Werror $< -o $@
$(BUILD_DIR)/stack_overflow.o: stack_overflow.c Makefile
	$(CC) -c $(CFLAGS) $< -o $@

$(BUILD_DIR)/client.elf: $(BUILD_DIR)/client.o
	$(LD) $(LDFLAGS) $^ $(LIBS) -o $@
$(BUILD_DIR)/server.elf: $(BUILD_DIR)/server.o
	$(LD) $(LDFLAGS) $^ $(LIBS) -o $@
$(BUILD_DIR)/hpet_timer.elf: $(BUILD_DIR)/hpet_timer.o
	$(LD) $(LDFLAGS) $^ $(LIBS) -o $@
$(BUILD_DIR)/stack_overflow.elf: $(BUILD_DIR)/stack_overflow.o
	$(LD) $(LDFLAGS) $^ $(LIBS) -o $@

$(INITIAL_TASK): $(addprefix $(BUILD_DIR)/, $(IMAGES)) hello.system
	$(MICROKIT_TOOL) hello.system --search-path $(BUILD_DIR) --board $(MICROKIT_BOARD) --config $(MICROKIT_CONFIG) -o $(INITIAL_TASK) -r $(REPORT)

$(COPIED_KERNEL): $(KERNEL)
	$(OBJCOPY) -O elf32-i386 $(KERNEL) $(COPIED_KERNEL)

qemu:
	qemu-system-x86_64                      \
			-cpu Nehalem,+fsgsbase,+pdpe1gb,+pcid,+invpcid,+xsave,+xsaves,+xsaveopt \
			-m "4G"                         \
			-display none                   \
			-serial mon:stdio               \
		-kernel $(COPIED_KERNEL) \
		-initrd $(INITIAL_TASK)

