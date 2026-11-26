/*
 * Copyright 2026, UNSW
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <microkit.h>

#if !defined(CONFIG_RISCV_AIA) || !defined(CONFIG_RISCV_IOMMU)
#error "This example requires KernelRiscvAIA and KernelIOMMU to be enabled."
#endif

#define PCI_COMMAND                 0x4u
#define PCI_STATUS                  0x6u
#define PCI_BAR0                    0x10u
#define PCI_CAP_PTR                 0x34u
#define PCI_COMMAND_MEMORY          0x2u
#define PCI_COMMAND_MASTER          0x4u
#define PCI_COMMAND_INTX_DISABLE    0x400u
#define PCI_STATUS_CAP_LIST         0x10u
#define PCI_CAP_ID_MSI              0x5u
#define PCI_MSI_FLAGS               0x2u
#define PCI_MSI_ADDRESS_LO          0x4u
#define PCI_MSI_ADDRESS_HI          0x8u
#define PCI_MSI_DATA_64             0xcu
#define PCI_MSI_ENABLE              0x1u
#define PCI_MSI_64BIT               0x80u

#define EDU_DEVICE_ID               0xedu
#define EDU_PCI_VENDOR_ID           0x1234u
#define EDU_PCI_DEVICE_ID           0x11e8u

#define EDU_MMIO_PADDR              0x40000000ull
#define EDU_MMIO_SIZE               0x00100000ull

#define EDU_BAR_SIZE                0x00100000ull
#define EDU_DMA_SRAM                0x40000ull
#define DMA_BUFFER_IOVA             0x00100000ull
#define MSI_IOVA                    0x00300000ull
#define MSI_IDENTITY                63u
#define MSI_CHANNEL                 0u
#define DMA_TEST_BYTES              64u
#define DMA_RETURN_OFFSET           0x100u
#define QEMU_TIMEBASE_HZ            10000000ull
#define DMA_TIMEOUT_TICKS           (2 * QEMU_TIMEBASE_HZ)

#define EDU_REG_ID                  0x00u
#define EDU_REG_ALIVE               0x04u
#define EDU_REG_IRQ_STATUS          0x24u
#define EDU_REG_IRQ_ACK             0x64u
#define EDU_REG_DMA_SRC             0x80u
#define EDU_REG_DMA_DST             0x88u
#define EDU_REG_DMA_COUNT           0x90u
#define EDU_REG_DMA_CMD             0x98u

#define EDU_DMA_START               0x1u
#define EDU_DMA_TO_RAM              0x2u
#define EDU_DMA_IRQ                 0x4u
#define EDU_IRQ_DMA                 0x100u

uint64_t edu_mmio_window_vaddr;
uint64_t dma_buffer_vaddr;
uint64_t pci_config_vaddr;
uint64_t edu_base;
uint64_t completions;

static void put_hex64(uint64_t value)
{
    char buf[19] = "0x0000000000000000";
    for (size_t i = 0; i < 16; i++) {
        unsigned int nibble = (value >> ((15 - i) * 4)) & 0xf;
        buf[2 + i] = nibble < 10 ? '0' + nibble : 'a' + (nibble - 10);
    }
    microkit_dbg_puts(buf);
}

static uint32_t pci_config_read32(uint8_t offset) {
    return *(volatile uint32_t *)(pci_config_vaddr + offset);
}

static uint32_t pci_config_read16(uint8_t offset) {
    return *(volatile uint16_t *)(pci_config_vaddr + offset);
}

static uint8_t pci_config_read8(uint8_t offset) {
    return *(volatile uint8_t *)(pci_config_vaddr + offset);
}

static void pci_config_write32(uint8_t offset, uint32_t value) {
    *(volatile uint32_t *)(pci_config_vaddr + offset) = value;
}
static void pci_config_write16(uint8_t offset, uint16_t value) {
    *(volatile uint16_t *)(pci_config_vaddr + offset) = value;
}

static void fence_io() {
    asm volatile("fence iorw, iorw" ::: "memory");
}

static bool configure_msi() {
    if (!(pci_config_read16(PCI_STATUS) & PCI_STATUS_CAP_LIST)) {
        return false;
    }
    uint8_t cap = pci_config_read8(PCI_CAP_PTR);
    for (size_t n = 0; cap >= 0x40 && n < 48; n++) {
        if (cap & 3) {
            return false;
        }
        if (pci_config_read8(cap) == PCI_CAP_ID_MSI) {
            uint16_t flags = pci_config_read16(cap + PCI_MSI_FLAGS);
            if (!(flags & PCI_MSI_64BIT) || cap > 0xf0) {
                return false;
            }
            pci_config_write16(cap + PCI_MSI_FLAGS, flags & ~PCI_MSI_ENABLE);
            pci_config_write32(cap + PCI_MSI_ADDRESS_LO, (uint32_t)MSI_IOVA);
            pci_config_write32(cap + PCI_MSI_ADDRESS_HI, (uint32_t)(MSI_IOVA >> 32));
            pci_config_write16(cap + PCI_MSI_DATA_64, MSI_IDENTITY);
            fence_io();
            pci_config_write16(cap + PCI_MSI_FLAGS, flags | PCI_MSI_ENABLE);
            fence_io();
            return (pci_config_read16(cap + PCI_MSI_FLAGS) & PCI_MSI_ENABLE) != 0;
        }
        cap = pci_config_read8(cap + 1);
    }
    return false;
}

static uint64_t read_time() {
    uint64_t value;
    asm volatile("rdtime %0" : "=r"(value));
    return value;
}

static volatile uint32_t *edu_reg32(uintptr_t edu_base, uint32_t offset) {
    return (volatile uint32_t *)(edu_base + offset);
}

static volatile uint64_t *edu_reg64(uintptr_t edu_base, uint32_t offset) {
    return (volatile uint64_t *)(edu_base + offset);
}

static uint32_t edu_read32(uintptr_t edu_base, uint32_t offset)
{
    return *edu_reg32(edu_base, offset);
}

static void edu_write32(uintptr_t edu_base, uint32_t offset, uint32_t value)
{
    *edu_reg32(edu_base, offset) = value;
}

static void edu_write64(uintptr_t edu_base, uint32_t offset, uint64_t value)
{
    *edu_reg64(edu_base, offset) = value;
}

static bool wait_for_dma(uintptr_t edu_base) {
    uint64_t start = read_time();
    while (edu_read32(edu_base, EDU_REG_DMA_CMD) & EDU_DMA_START) {
        if (read_time() - start >= DMA_TIMEOUT_TICKS) {
            microkit_dbg_puts("riscv64_iommu_dma_test: DMA timed out \n");
            return false;
        }
    }
    fence_io();
    return true;
}

static void fill_dma_buffer() {
    volatile uint32_t *buf = (volatile uint32_t *)dma_buffer_vaddr;

    for (size_t i = 0; i < DMA_TEST_BYTES / sizeof(uint32_t); i++) {
        buf[i] = 0x5a000000u | (uint32_t)i;
        buf[(DMA_RETURN_OFFSET / sizeof(uint32_t)) + i] = 0;
    }
}

static bool check_dma_return_buffer() {
    volatile uint32_t *buf = (volatile uint32_t *)dma_buffer_vaddr;

    for (size_t i = 0; i < DMA_TEST_BYTES / sizeof(uint32_t); i++) {
        uint32_t expected = 0x5a000000u | (uint32_t)i;
        uint32_t actual = buf[(DMA_RETURN_OFFSET / sizeof(uint32_t)) + i];
        if (actual != expected) {
            microkit_dbg_puts("DMA return buffer mismatch at word ");
            put_hex64(i);
            microkit_dbg_puts(": expected ");
            put_hex64(expected);
            microkit_dbg_puts(", got ");
            put_hex64(actual);
            microkit_dbg_puts("\n");
            return false;
        }
    }
    return true;
}

static bool check_edu_liveness(uintptr_t edu_base) {
    const uint32_t probe = 0xa5a55a5au;

    edu_write32(edu_base, EDU_REG_ALIVE, probe);
    fence_io();
    return edu_read32(edu_base, EDU_REG_ALIVE) == ~probe;
}

void init() {
    microkit_dbg_puts("riscv64_iommu_dma_test: starting\n");
    uint32_t id = pci_config_read32(0x00);
    if ((id & 0xFFFFu) != EDU_PCI_VENDOR_ID || ((id >> 16) & 0xFFFFu) != EDU_PCI_DEVICE_ID) {
        microkit_dbg_puts("riscv64_iommu_dma_test: EDU PCI device not found, id=");
        put_hex64(id);
        microkit_dbg_puts("\n");
        return;
    }
    pci_config_write32(PCI_BAR0, EDU_MMIO_PADDR);

    fence_io();
    uint64_t bar0 = pci_config_read32(PCI_BAR0) & ~0xfu;
    if (bar0 < EDU_MMIO_PADDR || bar0 + EDU_BAR_SIZE > EDU_MMIO_PADDR + EDU_MMIO_SIZE) {
        microkit_dbg_puts("riscv64_iommu_dma_test: EDU BAR outside mapped MMIO window, bar=");
        put_hex64(bar0);
        microkit_dbg_puts("\n");
        return;
    }

    uint16_t command = pci_config_read16(PCI_COMMAND);
    command |= PCI_COMMAND_MASTER | PCI_COMMAND_MEMORY | PCI_COMMAND_INTX_DISABLE;
    pci_config_write16(PCI_COMMAND, command);
    fence_io();

    edu_base = (uintptr_t)edu_mmio_window_vaddr + (uintptr_t)(bar0 - EDU_MMIO_PADDR);
    microkit_dbg_puts("riscv64_iommu_dma_test: EDU BAR=");
    put_hex64(bar0);
    microkit_dbg_puts(", id_reg=");
    uint32_t device_id = edu_read32(edu_base, EDU_REG_ID);
    put_hex64(device_id);
    if ((device_id & 0xFF) != EDU_DEVICE_ID) {
        microkit_dbg_puts("riscv64_iommu_dma_test: Identification failed.\n");
        return;
    }
    microkit_dbg_puts("\n");

    if (!check_edu_liveness(edu_base)) {
        microkit_dbg_puts("riscv64_iommu_dma_test: EDU MMIO liveness check failed\n");
        return;
    }
    fill_dma_buffer();
    edu_write64(edu_base, EDU_REG_DMA_SRC, DMA_BUFFER_IOVA);
    edu_write64(edu_base, EDU_REG_DMA_DST, EDU_DMA_SRAM);
    edu_write64(edu_base, EDU_REG_DMA_COUNT, DMA_TEST_BYTES);
    fence_io();
    edu_write32(edu_base, EDU_REG_DMA_CMD, EDU_DMA_START);
    fence_io();

    if (!wait_for_dma(edu_base)) {
        return;
    }

    if (!configure_msi()) {
        microkit_dbg_puts("riscv64_iommu_dma_test: MSI configuration failed\n");
        return;
    }

    edu_write32(edu_base, EDU_REG_IRQ_ACK, edu_read32(edu_base, EDU_REG_IRQ_STATUS));
    fence_io();

    edu_write64(edu_base, EDU_REG_DMA_SRC, EDU_DMA_SRAM);
    edu_write64(edu_base, EDU_REG_DMA_DST, DMA_BUFFER_IOVA + DMA_RETURN_OFFSET);
    edu_write64(edu_base, EDU_REG_DMA_COUNT, DMA_TEST_BYTES);
    fence_io();

    edu_write32(edu_base, EDU_REG_DMA_CMD, EDU_DMA_START | EDU_DMA_TO_RAM | EDU_DMA_IRQ);
    fence_io();
    microkit_dbg_puts("riscv64_iommu_dma_test: waiting for MSI to indicate DMA completion\n");
}

void notified(microkit_channel ch) {
    if (ch != MSI_CHANNEL) {
        return;
    }
    uint32_t status = edu_read32(edu_base, EDU_REG_IRQ_STATUS);
    if (!(status & EDU_IRQ_DMA) || (edu_read32(edu_base, EDU_REG_DMA_CMD) & EDU_DMA_START)) {
        microkit_dbg_puts("riscv64_iommu_dma_test: unexpected MSI\n");
        return;
    }
    fence_io();
    if (!check_dma_return_buffer()) {
        microkit_dbg_puts("riscv64_iommu_dma_test: DMA verification failed\n");
        return;
    }
    edu_write32(edu_base, EDU_REG_IRQ_ACK, status);
    fence_io();
    microkit_irq_ack(ch);

    if (++completions == 2) {
        microkit_dbg_puts("riscv64_iommu_dma_test: DMA verification passed\n");
        return;
    }

    fill_dma_buffer();
    fence_io();
    edu_write32(edu_base, EDU_REG_DMA_CMD, EDU_DMA_START | EDU_DMA_TO_RAM | EDU_DMA_IRQ);
    fence_io();
}

