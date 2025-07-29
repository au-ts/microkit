//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use crate::{
    capdl::{
        spec::{
            cap,
            object::{self, ArmIrqExtraInfo, SchedContextExtraInfo},
            Cap, CapTableEntry, FrameInit, NamedObject, Object, ObjectId, Rights,
        },
        CapDLSpec,
    },
    sdf::{SysIrq, SysIrqKind},
    sel4::{Arch, Config},
};

/// This module contains utility functions used by higher-level
/// CapDL spec generation code. For simplicity, this code will trust
/// all arguments given to it as it is only meant to be used internally
/// in the CapDL implementation.

/// Create a frame object and add it to the spec, returns the
/// object number.
pub fn capdl_util_make_frame_obj(
    spec: &mut CapDLSpec,
    frame_init: FrameInit,
    name: &str,
    paddr: Option<usize>,
    size_bits: usize,
) -> ObjectId {
    let frame_inner_obj = Object::Frame(object::Frame {
        size_bits: size_bits,
        paddr,
        init: frame_init,
    });
    let frame_obj = NamedObject {
        name: format!("frame_{}", name),
        object: frame_inner_obj,
    };
    spec.add_root_object(frame_obj)
}

/// Create a frame capability from a frame object for mapping the frame in a VSpace
pub fn capdl_util_make_frame_cap(
    frame_obj_id: ObjectId,
    read: bool,
    write: bool,
    execute: bool,
    cached: bool,
) -> Cap {
    Cap::Frame(cap::Frame {
        object: frame_obj_id,
        rights: Rights {
            read,
            write,
            // This is ignored on x86 by seL4. As the NX/XD bit that marks page as non-executable
            // is unsupported on old hardware.
            grant: execute,
            grant_reply: false,
        },
        cached,
    })
}

pub fn capdl_util_make_tcb_cap(tcb_obj_id: ObjectId) -> Cap {
    Cap::Tcb(cap::Tcb { object: tcb_obj_id })
}

pub fn capdl_util_make_page_table_cap(pt_obj_id: ObjectId) -> Cap {
    Cap::PageTable(cap::PageTable { object: pt_obj_id })
}

// Given a TCB object ID, return that TCB's VSpace object ID.
pub fn capdl_util_get_vspace_id_from_tcb_id(spec: &CapDLSpec, tcb_obj_id: ObjectId) -> ObjectId {
    let tcb = match spec.get_root_object(tcb_obj_id) {
        Some(named_object) => {
            if let Object::Tcb(tcb) = &named_object.object {
                Some(tcb)
            } else {
                unreachable!("internal bug: get_vspace_id_from_tcb_id() got a non TCB object ID.");
            }
        }
        None => {
            unreachable!(
                "internal bug: get_vspace_id_from_tcb_id() couldn't find tcb with given obj id."
            );
        }
    };
    let vspace_cap = tcb.unwrap().slots.iter().find(|&cte| {
        if let Cap::PageTable(_) = &cte.1 {
            true
        } else {
            false
        }
    });
    vspace_cap.unwrap().1.obj()
}

pub fn capdl_util_make_endpoint_obj(
    spec: &mut CapDLSpec,
    pd_name: &str,
    is_fault: bool,
) -> ObjectId {
    let fault_ep_obj = NamedObject {
        name: format!("ep_{}{}", if is_fault { "fault_" } else { "" }, pd_name).to_string(),
        object: Object::Endpoint,
    };
    spec.add_root_object(fault_ep_obj)
}

pub fn capdl_util_make_endpoint_cap(
    ep_obj_id: ObjectId,
    read: bool,
    write: bool,
    grant: bool,
    badge: u64,
) -> Cap {
    Cap::Endpoint(cap::Endpoint {
        object: ep_obj_id,
        badge,
        rights: Rights {
            read,
            write,
            grant,
            grant_reply: false,
        },
    })
}

pub fn capdl_util_make_ntfn_obj(spec: &mut CapDLSpec, pd_name: &str) -> ObjectId {
    let ntfn_obj = NamedObject {
        name: format!("ntfn_{}", pd_name),
        object: Object::Notification,
    };
    spec.add_root_object(ntfn_obj)
}

pub fn capdl_util_make_ntfn_cap(ntfn_obj_id: ObjectId, read: bool, write: bool, badge: u64) -> Cap {
    Cap::Notification(cap::Notification {
        object: ntfn_obj_id,
        badge: badge,
        rights: Rights {
            read,
            write,
            // Irrelevant for notifications, seL4 manual v13.0.0 pg11
            grant: false,
            grant_reply: false,
        },
    })
}

pub fn capdl_util_make_reply_obj(spec: &mut CapDLSpec, pd_name: &str) -> ObjectId {
    let reply_obj = NamedObject {
        name: format!("reply_{}", pd_name).to_string(),
        object: Object::Reply,
    };
    spec.add_root_object(reply_obj)
}

pub fn capdl_util_make_reply_cap(reply_obj_id: ObjectId) -> Cap {
    Cap::Reply(cap::Reply {
        object: reply_obj_id,
    })
}

pub fn capdl_util_make_sc_obj(
    spec: &mut CapDLSpec,
    pd_name: &str,
    size_bits: usize,
    period: u64,
    budget: u64,
    badge: u64,
) -> ObjectId {
    let sc_inner_obj = Object::SchedContext(object::SchedContext {
        size_bits,
        extra: SchedContextExtraInfo {
            period,
            budget,
            badge,
        },
    });
    let sc_obj = NamedObject {
        name: format!("sched_context_{}", pd_name).to_string(),
        object: sc_inner_obj,
    };
    spec.add_root_object(sc_obj)
}

pub fn capdl_util_make_sc_cap(sc_obj_id: ObjectId) -> Cap {
    Cap::SchedContext(cap::SchedContext { object: sc_obj_id })
}

pub fn capdl_util_make_cnode_obj(
    spec: &mut CapDLSpec,
    pd_name: &str,
    size_bits: usize,
    slots: Vec<CapTableEntry>,
) -> ObjectId {
    let cnode_inner_obj = Object::CNode(object::CNode { size_bits, slots });
    let cnode_obj = NamedObject {
        name: format!("cnode_{}", pd_name).to_string(),
        object: cnode_inner_obj,
    };
    // Move monitor CSpace into spec and make a cap for it to insert into TCB later.
    spec.add_root_object(cnode_obj)
}

pub fn capdl_util_make_cnode_cap(cnode_obj_id: ObjectId, guard: u64, guard_size: u64) -> Cap {
    Cap::CNode(cap::CNode {
        object: cnode_obj_id,
        guard,

        guard_size,
    })
}

pub fn capdl_util_make_ioport_obj(
    spec: &mut CapDLSpec,
    pd_name: &str,
    start_addr: u64,
    size: u64,
) -> ObjectId {
    let ioport_inner_obj = Object::IOPorts(object::IOPorts {
        start_port: start_addr,
        end_port: start_addr + size - 1,
    });
    let ioport_obj = NamedObject {
        name: format!("ioports_0x{:x}_{}", start_addr, pd_name),
        object: ioport_inner_obj,
    };
    spec.add_root_object(ioport_obj)
}

pub fn capdl_util_make_ioport_cap(ioport_obj_id: ObjectId) -> Cap {
    Cap::IOPorts(cap::IOPorts {
        object: ioport_obj_id,
    })
}

pub fn capdl_util_insert_cap_into_cspace(
    spec: &mut CapDLSpec,
    cspace_obj_id: ObjectId,
    idx: usize,
    cap: Cap,
) {
    let cspace_obj = spec.get_root_object_mut(cspace_obj_id).unwrap();
    if let Object::CNode(cspace_inner_obj) = &mut cspace_obj.object {
        cspace_inner_obj.slots.push((idx, cap));
    } else {
        unreachable!("internal bug: capdl_util_insert_cap_into_cspace() got a non CNode object.");
    }
}

/// target_cpu is only valid for ARM.
pub fn capdl_util_make_irq_obj(
    spec: &mut CapDLSpec,
    sel4_config: &Config,
    pd_name: &str,
    sys_irq: &SysIrq,
    target_cpu: Option<u64>,
) -> ObjectId {
    let irq_inner_obj = match sys_irq.kind {
        SysIrqKind::Conventional { trigger, .. } => match sel4_config.arch {
            Arch::Aarch64 => Object::ArmIrq(object::ArmIrq {
                slots: [].to_vec(),
                extra: ArmIrqExtraInfo {
                    trigger: trigger as u64,
                    target: target_cpu.unwrap(),
                },
            }),
            Arch::Riscv64 => Object::Irq(object::Irq {
                slots: [].to_vec(),
            }),
            Arch::X86_64 => unreachable!("internal bug: ARM and RISC-V IRQs not supported on x86."),
        },

        SysIrqKind::IOAPIC {
            ioapic,
            pin,
            level,
            polarity,
            ..
        } => Object::IrqIOApic(object::IrqIOApic {
            slots: [].to_vec(),
            extra: object::IrqIOApicExtraInfo {
                ioapic,
                pin,
                level,
                polarity,
            },
        }),
        SysIrqKind::MSI {
            pci_bus,
            pci_dev,
            pci_func,
            handle,
            ..
        } => Object::IrqMsi(object::IrqMsi {
            slots: [].to_vec(),
            extra: object::IrqMsiExtraInfo {
                handle,
                pci_bus,
                pci_dev,
                pci_func,
            },
        }),
    };
    let irq_obj = NamedObject {
        name: format!("irq_{}_{}", sys_irq.irq_num(), pd_name),
        object: irq_inner_obj,
    };
    spec.add_root_object(irq_obj)
}

pub fn capdl_util_make_irq_handler_cap(irq_obj_id: ObjectId, irq_kind: &SysIrqKind) -> Cap {
    // Look up what kind of IRQ we are dealing with to create the correct cap type
    match irq_kind {
        SysIrqKind::Conventional { .. } => {
            Cap::ArmIrqHandler(cap::ArmIrqHandler { object: irq_obj_id })
        }
        SysIrqKind::IOAPIC { .. } => {
            Cap::IrqIOApicHandler(cap::IrqIOApicHandler { object: irq_obj_id })
        }
        SysIrqKind::MSI { .. } => Cap::IrqMsiHandler(cap::IrqMsiHandler { object: irq_obj_id }),
    }
}

pub fn capdl_util_bind_irq_to_ntfn(spec: &mut CapDLSpec, irq_obj_id: ObjectId, ntfn_cap: Cap) {
    match &mut spec.get_root_object_mut(irq_obj_id).unwrap().object {
        Object::ArmIrq(arm_irq) => {
            arm_irq.slots.push((0, ntfn_cap));
        }
        Object::IrqMsi(irq_msi) => {
            irq_msi.slots.push((0, ntfn_cap));
        }
        Object::IrqIOApic(irq_ioapic) => {
            irq_ioapic.slots.push((0, ntfn_cap));
        }
        Object::Irq(generic_irq) => {
            generic_irq.slots.push((0, ntfn_cap));
        }
        _ => unreachable!("internal bug: capdl_util_bind_irq_to_ntfn() got non irq object"),
    }
}

pub fn capdl_util_make_vcpu_obj(spec: &mut CapDLSpec, name: &String) -> ObjectId {
    let vcpu_inner_obj = Object::VCpu;
    let vcpu_obj = NamedObject {
        name: format!("vcpu_{}", name).to_string(),
        object: vcpu_inner_obj,
    };
    spec.add_root_object(vcpu_obj)
}

pub fn capdl_util_make_vcpu_cap(vcpu_obj_id: ObjectId) -> Cap {
    Cap::VCpu(cap::VCpu {
        object: vcpu_obj_id,
    })
}

pub fn capdl_util_make_arm_smc_cap(arm_smc_obj_id: ObjectId) -> Cap {
    Cap::ArmSmc(cap::ArmSmc {
        object: arm_smc_obj_id,
    })
}
