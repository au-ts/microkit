//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use std::{fs::File, io::Write};

use crate::{
    capdl::{spec::CapDLObject, CapDLSpec},
    sel4::{Arch, Config},
};

pub fn write_report(spec: &CapDLSpec, kernel_config: &Config, output_path: &str) {
    let mut report_file = File::create(output_path).expect("Cannot create report file");

    report_file
        .write_all(b"# Initial Task (CapDL Initialiser) Details\n")
        .unwrap();

    report_file.write_all(b"# IRQ Details\n").unwrap();
    for irq in spec.irqs.iter() {
        let irq_num = irq.irq;
        let handler = spec.get_root_object(irq.handler).unwrap();

        match &handler.object {
            CapDLObject::ArmIrq(arm_irq) => {
                report_file
                    .write_all(
                        format!("\t- IRQ Number: {}, handler: {}\n", irq.irq, handler.name)
                            .as_bytes(),
                    )
                    .unwrap();
            }
            CapDLObject::RiscvIrq(riscv_irq) => todo!(),
            CapDLObject::IrqMsi(irq_msi) => todo!(),
            CapDLObject::IrqIOApic(irq_ioapic) => todo!(),
            _ => unreachable!("internal bug: object is not IRQ!"),
        };
    }

    report_file.write_all(b"# TCB Details\n").unwrap();

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
