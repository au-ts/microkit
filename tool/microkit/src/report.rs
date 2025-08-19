//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use std::{fs::File, io::Write};

use crate::{
    capdl::{spec::CapDLObject, CapDLSpec},
    sel4::{ArmRiscvIrqTrigger, Config, X86IoapicIrqPolarity, X86IoapicIrqTrigger},
};

pub fn write_report(spec: &CapDLSpec, kernel_config: &Config, output_path: &str) {
    let mut report_file = File::create(output_path).expect("Cannot create report file");

    report_file
        .write_all(b"# Initial Task (CapDL Initialiser) Details\n")
        .unwrap();

    report_file.write_all(b"\n# IRQ Details\n").unwrap();
    for irq in spec.irqs.iter() {
        let irq_num = irq.irq;
        let handler = spec.get_root_object(irq.handler).unwrap();

        match &handler.object {
            CapDLObject::ArmIrq(arm_irq) => {
                report_file
                    .write_all(format!("\t- IRQ Number: {}\n", irq_num).as_bytes())
                    .unwrap();
                report_file
                    .write_all(
                        format!(
                            "\t\t* Trigger: {}\n",
                            ArmRiscvIrqTrigger::from(arm_irq.extra.trigger).human_name()
                        )
                        .as_bytes(),
                    )
                    .unwrap();
                report_file
                    .write_all(format!("\t\t* CPU: {}\n", arm_irq.extra.target).as_bytes())
                    .unwrap();
            }
            CapDLObject::RiscvIrq(riscv_irq) => {
                report_file
                    .write_all(format!("\t- IRQ Number: {}\n", irq_num).as_bytes())
                    .unwrap();
                report_file
                    .write_all(
                        format!(
                            "\t\t* Trigger: {}\n",
                            ArmRiscvIrqTrigger::from(riscv_irq.extra.trigger).human_name()
                        )
                        .as_bytes(),
                    )
                    .unwrap();
            }
            CapDLObject::IrqMsi(irq_msi) => {
                report_file
                    .write_all(format!("\t- IRQ Vector: {}\n", irq_num).as_bytes())
                    .unwrap();
                report_file
                    .write_all(format!("\t\t* PCI Bus: {}\n", irq_msi.extra.pci_bus).as_bytes())
                    .unwrap();
                report_file
                    .write_all(format!("\t\t* PCI Device: {}\n", irq_msi.extra.pci_dev).as_bytes())
                    .unwrap();
                report_file
                    .write_all(
                        format!("\t\t* PCI Function: {}\n", irq_msi.extra.pci_func).as_bytes(),
                    )
                    .unwrap();
                report_file
                    .write_all(format!("\t\t* Handle: {}\n", irq_msi.extra.handle).as_bytes())
                    .unwrap();
            }
            CapDLObject::IrqIOApic(irq_ioapic) => {
                report_file
                    .write_all(format!("\t- IRQ Vector: {}\n", irq_num).as_bytes())
                    .unwrap();
                report_file
                    .write_all(format!("\t\t* IOAPIC: {}\n", irq_ioapic.extra.ioapic).as_bytes())
                    .unwrap();
                report_file
                    .write_all(format!("\t\t* Pin: {}\n", irq_ioapic.extra.pin).as_bytes())
                    .unwrap();
                report_file
                    .write_all(
                        format!(
                            "\t\t* Trigger: {}\n",
                            X86IoapicIrqTrigger::from(irq_ioapic.extra.level).human_name()
                        )
                        .as_bytes(),
                    )
                    .unwrap();
                report_file
                    .write_all(
                        format!(
                            "\t\t* Polarity: {}\n",
                            X86IoapicIrqPolarity::from(irq_ioapic.extra.polarity).human_name()
                        )
                        .as_bytes(),
                    )
                    .unwrap();
            }
            _ => unreachable!("internal bug: object is not IRQ!"),
        };
    }

    report_file.write_all(b"\n# TCB Details\n").unwrap();

    report_file.write_all(b"# CNode Details\n").unwrap();

    report_file
        .write_all(b"# Architecture Specific Details\n")
        .unwrap();

    report_file
        .write_all(b"# Kernel Objects Details\n")
        .unwrap();
    let kernel_objects = spec
        .objects
        .iter()
        .filter(|named_object| named_object.object.physical_size_bits(kernel_config) > 0);
    for named_object in kernel_objects {
        match &named_object.expected_alloc {
            Some(allocation_details) => {
                report_file
                    .write_all(
                        format!(
                            "\t{}: name: {}\tphys_addr: 0x{:0>12x}\n",
                            named_object.object.human_name(kernel_config),
                            named_object.name,
                            allocation_details.paddr
                        )
                        .as_bytes(),
                    )
                    .unwrap();
            }
            None => {
                report_file
                    .write_all(
                        format!(
                            "\t{}: name: {}\n",
                            named_object.object.human_name(kernel_config),
                            named_object.name
                        )
                        .as_bytes(),
                    )
                    .unwrap();
            }
        };
    }
}
