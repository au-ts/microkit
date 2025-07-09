//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//
use core::ops::Range;
use sel4_capdl_initializer_types::Word;
use serde::{Deserialize, Serialize};

use crate::sel4::{Config, ObjectType};

pub type ObjectId = usize;
pub type Badge = Word;
pub type CPtr = Word;
pub type CapSlot = usize;
pub type CapTableEntry = (CapSlot, Cap);

// CapDL Spec objects
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct IrqEntry {
    pub irq: Word,
    pub handler: ObjectId,
}

pub type AsidSlotEntry = ObjectId;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct UntypedCover {
    pub parent: ObjectId,
    pub children: Range<ObjectId>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct NamedObject {
    pub name: String,
    pub object: Object,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub enum FrameInit {
    Fill(Fill),
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct Fill {
    pub entries: Vec<FillEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub enum FillEntryContent {
    Data(FileContentRange),
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct FillEntry {
    pub range: Range<usize>,
    pub content: FillEntryContent,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct FileContentRange {
    pub file: String,
    pub file_offset: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub enum Object {
    // Untyped(object::Untyped),
    Endpoint,
    Notification,
    CNode(object::CNode),
    Tcb(object::Tcb),
    Irq(object::Irq),
    VCpu,
    Frame(object::Frame),
    PageTable(object::PageTable),
    AsidPool(object::AsidPool),
    ArmIrq(object::ArmIrq),
    IrqMsi(object::IrqMsi),
    IrqIOApic(object::IrqIOApic),
    IOPorts(object::IOPorts),
    SchedContext(object::SchedContext),
    Reply,
}

impl Object {
    pub fn paddr(&self) -> Option<usize> {
        match self {
            // Object::Untyped(obj) => obj.paddr,
            Object::Frame(obj) => obj.paddr,
            _ => None,
        }
    }
    
    /// CNode and SchedContext are quirky as they have variable size.
    pub fn physical_size_bits(&self, sel4_config: &Config) -> u64 {
        match self {
            Object::Endpoint => ObjectType::Endpoint.fixed_size_bits(sel4_config).unwrap(),
            Object::Notification => ObjectType::Notification.fixed_size_bits(sel4_config).unwrap(),
            Object::CNode(cnode) => cnode.size_bits as u64 + 5, // @billn figure out where RHS come from
            Object::Tcb(_) => ObjectType::Tcb.fixed_size_bits(sel4_config).unwrap(),
            Object::Irq(_) => 0,
            Object::VCpu => ObjectType::Vcpu.fixed_size_bits(sel4_config).unwrap(),
            Object::Frame(frame) => frame.size_bits as u64,
            Object::PageTable(_) => ObjectType::PageTable.fixed_size_bits(sel4_config).unwrap(),
            Object::AsidPool(_) => 0, // This isn't zero on x86, is it also zero on arm?? @billn revisit
            Object::ArmIrq(_) => 0,
            Object::IrqMsi(_) => 0,
            Object::IrqIOApic(_) => 0,
            Object::IOPorts(_) => 0,
            Object::SchedContext(sched_context) => sched_context.size_bits as u64, // @billn fix, there should be a base size
            Object::Reply => ObjectType::Reply.fixed_size_bits(sel4_config).unwrap(),
        }
    }

    pub fn get_cap_entries_mut(&mut self) -> Option<&mut Vec<CapTableEntry>> {
        match self {
            Object::CNode(cnode) => Some(&mut cnode.slots),
            Object::Tcb(tcb) => Some(&mut tcb.slots),
            Object::Irq(irq) => Some(&mut irq.slots),
            Object::PageTable(page_table) => Some(&mut page_table.slots),
            Object::ArmIrq(arm_irq) => Some(&mut arm_irq.slots),
            Object::IrqMsi(irq_msi) => Some(&mut irq_msi.slots),
            Object::IrqIOApic(irq_ioapic) => Some(&mut irq_ioapic.slots),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub enum Cap {
    // Untyped(cap::Untyped),
    Endpoint(cap::Endpoint),
    Notification(cap::Notification),
    CNode(cap::CNode),
    Tcb(cap::Tcb),
    IrqHandler(cap::IrqHandler),
    VCpu(cap::VCpu),
    Frame(cap::Frame),
    PageTable(cap::PageTable),
    AsidPool(cap::AsidPool),
    ArmIrqHandler(cap::ArmIrqHandler),
    IrqMsiHandler(cap::IrqMsiHandler),
    IrqIOApicHandler(cap::IrqIOApicHandler),
    IOPorts(cap::IOPorts),
    SchedContext(cap::SchedContext),
    Reply(cap::Reply),
}

impl Cap {
    pub fn obj(&self) -> ObjectId {
        match self {
            // @billn do we ever need to use the untyped object in spec generation?
            // Cap::Untyped(cap) => cap.object,
            Cap::Endpoint(cap) => cap.object,
            Cap::Notification(cap) => cap.object,
            Cap::CNode(cap) => cap.object,
            Cap::Frame(cap) => cap.object,
            Cap::Tcb(cap) => cap.object,
            Cap::IrqHandler(cap) => cap.object,
            Cap::VCpu(cap) => cap.object,
            Cap::PageTable(cap) => cap.object,
            Cap::AsidPool(cap) => cap.object,
            Cap::ArmIrqHandler(cap) => cap.object,
            Cap::IrqMsiHandler(cap) => cap.object,
            Cap::IrqIOApicHandler(cap) => cap.object,
            Cap::IOPorts(cap) => cap.object,
            Cap::SchedContext(cap) => cap.object,
            Cap::Reply(cap) => cap.object,
        }
    }

    pub fn set_id(&mut self, new_id: ObjectId) {
        match self {
            // @billn do we ever need to use the untyped object in spec generation?
            // Cap::Untyped(cap) => cap.object,
            Cap::Endpoint(cap) => cap.object = new_id,
            Cap::Notification(cap) => cap.object = new_id,
            Cap::CNode(cap) => cap.object = new_id,
            Cap::Frame(cap) => cap.object = new_id,
            Cap::Tcb(cap) => cap.object = new_id,
            Cap::IrqHandler(cap) => cap.object = new_id,
            Cap::VCpu(cap) => cap.object = new_id,
            Cap::PageTable(cap) => cap.object = new_id,
            Cap::AsidPool(cap) => cap.object = new_id,
            Cap::ArmIrqHandler(cap) => cap.object = new_id,
            Cap::IrqMsiHandler(cap) => cap.object = new_id,
            Cap::IrqIOApicHandler(cap) => cap.object = new_id,
            Cap::IOPorts(cap) => cap.object = new_id,
            Cap::SchedContext(cap) => cap.object = new_id,
            Cap::Reply(cap) => cap.object = new_id,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct Rights {
    pub read: bool,
    pub write: bool,
    pub grant: bool,
    pub grant_reply: bool,
}

pub mod object {
    use super::*;

    // #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    // pub struct Untyped {
    //     pub size_bits: usize,
    //     pub paddr: Option<usize>,
    // }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct CNode {
        /// This is in addition to 
        pub size_bits: usize,
        pub slots: Vec<CapTableEntry>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct Tcb {
        pub slots: Vec<CapTableEntry>,
        pub extra: TcbExtraInfo,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
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

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct Irq {
        pub slots: Vec<CapTableEntry>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct Frame {
        pub size_bits: usize,
        pub paddr: Option<usize>,
        pub init: FrameInit,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct PageTable {
        pub is_root: bool,
        pub level: Option<u8>,
        pub slots: Vec<CapTableEntry>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct AsidPool {
        pub high: Word,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct ArmIrq {
        pub slots: Vec<CapTableEntry>,
        pub extra: ArmIrqExtraInfo,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct ArmIrqExtraInfo {
        pub trigger: Word,
        pub target: Word,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct IrqMsi {
        pub slots: Vec<CapTableEntry>,
        pub extra: IrqMsiExtraInfo,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct IrqMsiExtraInfo {
        pub handle: Word,
        pub pci_bus: Word,
        pub pci_dev: Word,
        pub pci_func: Word,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct IrqIOApic {
        pub slots: Vec<CapTableEntry>,
        pub extra: IrqIOApicExtraInfo,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct IrqIOApicExtraInfo {
        pub ioapic: Word,
        pub pin: Word,
        pub level: Word,
        pub polarity: Word,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct IOPorts {
        pub start_port: Word,
        pub end_port: Word,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct SchedContext {
        pub size_bits: usize,
        pub extra: SchedContextExtraInfo,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct SchedContextExtraInfo {
        pub period: u64,
        pub budget: u64,
        pub badge: Badge,
    }
}

pub mod cap {
    use super::*;

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct Untyped {
        pub object: ObjectId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
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

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct Notification {
        pub object: ObjectId,
        pub badge: Badge,
        pub rights: Rights,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct CNode {
        pub object: ObjectId,
        pub guard: Word,
        pub guard_size: Word,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct Tcb {
        pub object: ObjectId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct IrqHandler {
        pub object: ObjectId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct VCpu {
        pub object: ObjectId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct Frame {
        pub object: ObjectId,
        pub rights: Rights,
        pub cached: bool,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct PageTable {
        pub object: ObjectId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct AsidPool {
        pub object: ObjectId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct ArmIrqHandler {
        pub object: ObjectId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct IrqMsiHandler {
        pub object: ObjectId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct IrqIOApicHandler {
        pub object: ObjectId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct IOPorts {
        pub object: ObjectId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct SchedContext {
        pub object: ObjectId,
    }

    #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
    pub struct Reply {
        pub object: ObjectId,
    }
}
