//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//
use core::ops::Range;
use sel4_capdl_initializer_types::Word;
use serde::{Deserialize, Serialize};

use crate::{
    capdl::SLOT_BITS,
    sel4::{Config, ObjectType},
};

pub type ObjectId = usize;
pub type Badge = Word;
pub type CPtr = Word;
pub type CapSlot = usize;
pub type CapTableEntry = (CapSlot, Cap);

// CapDL Spec objects

#[derive(Serialize, Clone, Eq, PartialEq)]
pub struct IrqEntry {
    pub irq: Word,
    pub handler: ObjectId,
}

pub type AsidSlotEntry = ObjectId;

#[derive(Serialize, Clone, Eq, PartialEq)]
pub struct UntypedCover {
    pub parent: ObjectId,
    pub children: Range<ObjectId>,
}

#[derive(Serialize, Clone, Eq, PartialEq)]
pub struct NamedObject {
    pub name: String,
    pub object: Object,
}

#[derive(Serialize, Clone, Eq, PartialEq)]
pub enum FrameInit {
    Fill(Fill),
}

#[derive(Serialize, Clone, Eq, PartialEq)]
pub struct Fill {
    pub entries: Vec<FillEntry>,
}

#[derive(Serialize, Clone, Eq, PartialEq)]
pub struct FillEntry {
    pub range: Range<usize>,
    pub content: FillEntryContent,
}

#[derive(Serialize, Clone, Eq, PartialEq)]
pub enum FillEntryContent {
    Data(ElfContent),
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ElfContent {
    pub elf_id: usize,
    pub elf_seg_idx: usize,
    pub elf_seg_data_range: Range<usize>,
}

#[derive(Serialize, Clone, Eq, PartialEq)]
pub enum Object {
    Endpoint,
    Notification,
    CNode(object::CNode),
    Tcb(object::Tcb),
    VCpu,
    Frame(object::Frame),
    PageTable(object::PageTable),
    AsidPool(object::AsidPool),
    ArmIrq(object::ArmIrq),
    IrqMsi(object::IrqMsi),
    IrqIOApic(object::IrqIOApic),
    RiscvIrq(object::RiscvIrq),
    IOPorts(object::IOPorts),
    SchedContext(object::SchedContext),
    Reply,
    ArmSmc,
}

impl Object {
    pub fn paddr(&self) -> Option<usize> {
        match self {
            Object::Frame(obj) => obj.paddr,
            _ => None,
        }
    }

    /// CNode and SchedContext are quirky as they have variable size.
    pub fn physical_size_bits(&self, sel4_config: &Config) -> u64 {
        match self {
            Object::Endpoint => ObjectType::Endpoint.fixed_size_bits(sel4_config).unwrap(),
            Object::Notification => ObjectType::Notification
                .fixed_size_bits(sel4_config)
                .unwrap(),
            Object::CNode(cnode) => cnode.size_bits as u64 + SLOT_BITS,
            Object::Tcb(_) => ObjectType::Tcb.fixed_size_bits(sel4_config).unwrap(),
            Object::VCpu => ObjectType::Vcpu.fixed_size_bits(sel4_config).unwrap(),
            Object::Frame(frame) => frame.size_bits as u64,
            Object::PageTable(pt) => {
                if pt.is_root {
                    ObjectType::VSpace.fixed_size_bits(sel4_config).unwrap()
                } else {
                    ObjectType::PageTable.fixed_size_bits(sel4_config).unwrap()
                }
            }
            Object::AsidPool(_) => ObjectType::AsidPool.fixed_size_bits(sel4_config).unwrap(),
            Object::SchedContext(sched_context) => sched_context.size_bits as u64,
            Object::Reply => ObjectType::Reply.fixed_size_bits(sel4_config).unwrap(),
            _ => 0,
        }
    }

    pub fn get_cap_entries_mut(&mut self) -> Option<&mut Vec<CapTableEntry>> {
        match self {
            Object::CNode(cnode) => Some(&mut cnode.slots),
            Object::Tcb(tcb) => Some(&mut tcb.slots),
            Object::PageTable(page_table) => Some(&mut page_table.slots),
            Object::ArmIrq(arm_irq) => Some(&mut arm_irq.slots),
            Object::IrqMsi(irq_msi) => Some(&mut irq_msi.slots),
            Object::IrqIOApic(irq_ioapic) => Some(&mut irq_ioapic.slots),
            Object::RiscvIrq(riscv_irq) => Some(&mut riscv_irq.slots),
            _ => None,
        }
    }
}

#[derive(Serialize, Clone, Eq, PartialEq)]
pub enum Cap {
    // Untyped(cap::Untyped),
    Endpoint(cap::Endpoint),
    Notification(cap::Notification),
    CNode(cap::CNode),
    Tcb(cap::Tcb),
    VCpu(cap::VCpu),
    Frame(cap::Frame),
    PageTable(cap::PageTable),
    AsidPool(cap::AsidPool),
    ArmIrqHandler(cap::ArmIrqHandler),
    IrqMsiHandler(cap::IrqMsiHandler),
    IrqIOApicHandler(cap::IrqIOApicHandler),
    RiscvIrqHandler(cap::RiscvIrqHandler),
    IOPorts(cap::IOPorts),
    SchedContext(cap::SchedContext),
    Reply(cap::Reply),
    ArmSmc(cap::ArmSmc),
}

impl Cap {
    pub fn obj(&self) -> ObjectId {
        match self {
            Cap::Endpoint(cap) => cap.object,
            Cap::Notification(cap) => cap.object,
            Cap::CNode(cap) => cap.object,
            Cap::Frame(cap) => cap.object,
            Cap::Tcb(cap) => cap.object,
            Cap::VCpu(cap) => cap.object,
            Cap::PageTable(cap) => cap.object,
            Cap::AsidPool(cap) => cap.object,
            Cap::ArmIrqHandler(cap) => cap.object,
            Cap::IrqMsiHandler(cap) => cap.object,
            Cap::IrqIOApicHandler(cap) => cap.object,
            Cap::RiscvIrqHandler(cap) => cap.object,
            Cap::IOPorts(cap) => cap.object,
            Cap::SchedContext(cap) => cap.object,
            Cap::Reply(cap) => cap.object,
            Cap::ArmSmc(cap) => cap.object,
        }
    }

    pub fn set_id(&mut self, new_id: ObjectId) {
        match self {
            Cap::Endpoint(cap) => cap.object = new_id,
            Cap::Notification(cap) => cap.object = new_id,
            Cap::CNode(cap) => cap.object = new_id,
            Cap::Frame(cap) => cap.object = new_id,
            Cap::Tcb(cap) => cap.object = new_id,
            Cap::VCpu(cap) => cap.object = new_id,
            Cap::PageTable(cap) => cap.object = new_id,
            Cap::AsidPool(cap) => cap.object = new_id,
            Cap::ArmIrqHandler(cap) => cap.object = new_id,
            Cap::IrqMsiHandler(cap) => cap.object = new_id,
            Cap::IrqIOApicHandler(cap) => cap.object = new_id,
            Cap::RiscvIrqHandler(cap) => cap.object = new_id,
            Cap::IOPorts(cap) => cap.object = new_id,
            Cap::SchedContext(cap) => cap.object = new_id,
            Cap::Reply(cap) => cap.object = new_id,
            Cap::ArmSmc(cap) => cap.object = new_id,
        }
    }
}

#[derive(Serialize, Clone, Eq, PartialEq)]
pub struct Rights {
    pub read: bool,
    pub write: bool,
    pub grant: bool,
    pub grant_reply: bool,
}

pub mod object {
    use super::*;
    /// Any object that takes a size bits is in addition to the base size

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct CNode {
        pub size_bits: usize,
        pub slots: Vec<CapTableEntry>,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct Tcb {
        pub slots: Vec<CapTableEntry>,
        pub extra: TcbExtraInfo,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct TcbExtraInfo {
        pub ipc_buffer_addr: Word,

        pub affinity: Word,
        pub prio: u8,
        pub max_prio: u8,
        pub resume: bool,

        pub ip: Word,
        pub sp: Word,
        pub gprs: Vec<Word>,

        pub master_fault_ep: Option<CPtr>,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct Frame {
        pub size_bits: usize,
        pub paddr: Option<usize>,
        pub init: FrameInit,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct PageTable {
        pub is_root: bool,
        pub level: Option<u8>,
        pub slots: Vec<CapTableEntry>,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct AsidPool {
        pub high: Word,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct ArmIrq {
        pub slots: Vec<CapTableEntry>,
        pub extra: ArmIrqExtraInfo,
    }

    // Revisit this when doing RISC-V, should just be the same and change the name to be something more universal
    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct ArmIrqExtraInfo {
        pub trigger: Word,
        pub target: Word, // target CPU
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct IrqMsi {
        pub slots: Vec<CapTableEntry>,
        pub extra: IrqMsiExtraInfo,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct IrqMsiExtraInfo {
        pub handle: Word,
        pub pci_bus: Word,
        pub pci_dev: Word,
        pub pci_func: Word,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct IrqIOApic {
        pub slots: Vec<CapTableEntry>,
        pub extra: IrqIOApicExtraInfo,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct IrqIOApicExtraInfo {
        pub ioapic: Word,
        pub pin: Word,
        pub level: Word,
        pub polarity: Word,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct RiscvIrq {
        pub slots: Vec<CapTableEntry>,
        pub extra: RiscvIrqExtraInfo,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct RiscvIrqExtraInfo {
        pub trigger: Word,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct IOPorts {
        pub start_port: Word,
        pub end_port: Word,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct SchedContext {
        pub size_bits: usize,
        pub extra: SchedContextExtraInfo,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct SchedContextExtraInfo {
        pub period: u64,
        pub budget: u64,
        pub badge: Badge,
    }
}

pub mod cap {
    use super::*;

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct Untyped {
        pub object: ObjectId,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct Endpoint {
        pub object: ObjectId,
        // TODO
        //   parse-capDL uses badge=0 to mean no badge. Is that good
        //   enough, or do we ever need to actually use the badge value '0'?
        // TODO
        //   Is it correct that these are ignored in the case of Tcb::SLOT_TEMP_FAULT_EP?
        pub badge: Badge,
        pub rights: Rights,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct Notification {
        pub object: ObjectId,
        pub badge: Badge,
        pub rights: Rights,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct CNode {
        pub object: ObjectId,
        pub guard: Word,
        pub guard_size: Word,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct Tcb {
        pub object: ObjectId,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct VCpu {
        pub object: ObjectId,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct Frame {
        pub object: ObjectId,
        pub rights: Rights,
        pub cached: bool,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct PageTable {
        pub object: ObjectId,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct AsidPool {
        pub object: ObjectId,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct ArmIrqHandler {
        pub object: ObjectId,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct IrqMsiHandler {
        pub object: ObjectId,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct IrqIOApicHandler {
        pub object: ObjectId,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct RiscvIrqHandler {
        pub object: ObjectId,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct IOPorts {
        pub object: ObjectId,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct SchedContext {
        pub object: ObjectId,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct Reply {
        pub object: ObjectId,
    }

    #[derive(Serialize, Clone, Eq, PartialEq)]
    pub struct ArmSmc {
        pub object: ObjectId,
    }
}
