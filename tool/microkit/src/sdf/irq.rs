//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use super::pci::PciDevice;

use crate::sel4::{ArmIrqTrigger, RiscvIrqTrigger, X86IoapicIrqPolarity, X86IoapicIrqTrigger};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, PartialEq, Eq)]
pub enum SysIrqKind {
    Arm {
        irq: u64,
        trigger: ArmIrqTrigger,
    },
    Riscv {
        irq: u64,
        trigger: RiscvIrqTrigger,
        source: Option<u64>,
    },
    /// x86-64 specific
    IOAPIC {
        ioapic: u64,
        pin: u64,
        trigger: X86IoapicIrqTrigger,
        polarity: X86IoapicIrqPolarity,
        vector: u64,
    },
    /// x86-64 specific
    MSI {
        pci_device: PciDevice,
        handle: u64,
        vector: u64,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct SysIrq {
    pub id: u64,
    pub kind: SysIrqKind,
}

impl SysIrq {
    pub fn irq_num(&self) -> u64 {
        match self.kind {
            SysIrqKind::Arm { irq, .. } | SysIrqKind::Riscv { irq, .. } => irq,
            SysIrqKind::IOAPIC { vector, .. } => vector,
            SysIrqKind::MSI { vector, .. } => vector,
        }
    }
}
