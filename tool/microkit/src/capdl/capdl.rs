//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//
use core::ops::Range;

use std::{
    cmp::min,
    path::{Path, PathBuf},
    u8,
};

use serde::{Deserialize, Serialize};

use crate::{
    capdl::{
        memory::{self, ArchMethods, X86_64},
        spec::{
            cap,
            object::{self, SchedContextExtraInfo},
            AsidSlotEntry, Cap, CapTableEntry, FileContentRange, Fill, FillEntry, FillEntryContent,
            FrameInit, IrqEntry, NamedObject, Object, ObjectId, Rights, UntypedCover,
        },
        util::{
            capdl_util_get_vspace_id_from_tcb_id, capdl_util_make_cnode_cap, capdl_util_make_cnode_obj, capdl_util_make_endpoint_cap, capdl_util_make_endpoint_obj, capdl_util_make_frame_cap, capdl_util_make_frame_obj, capdl_util_make_reply_cap, capdl_util_make_reply_obj, capdl_util_make_sc_cap, capdl_util_make_sc_obj
        },
    },
    elf::ElfFile,
    sdf::SystemDescription,
    sel4::{Config, PageSize},
    util::{self, round_down},
};

// Corresponds to the IPC buffer symbol in libmicrokit and the monitor
const SYMBOL_IPC_BUFFER: &str = "__sel4_ipc_buffer_obj";

const FAULT_BADGE: u64 = 1 << 62;
const PPC_BADGE: u64 = 1 << 63;

// The sel4-capdl-initialiser crate expects caps that you want to bind to a TCB to be at
// certain slots. From dep/rust-sel4/crates/sel4-capdl-initializer/types/src/cap_table.rs
const TCB_SLOT_CSPACE: u64 = 0;
const TCB_SLOT_VSPACE: u64 = 1;
const TCB_SLOT_IPC_BUFFER: u64 = 4;
const TCB_SLOT_FAULT_EP: u64 = 5;
const TCB_SLOT_SC: u64 = 6;
// const TCB_SLOT_TEMP_FAULT_EP: u64 = 7;
const TCB_SLOT_BOUND_NOTIFICATION: u64 = 8;
const SLOT_VCPU: u64 = 9; // @billn revisit sel4-capdl-initialiser. it doesnt support multiple vCPUs

// Where caps must be in a PD's CSpace
const INPUT_CAP_IDX: u64 = 1;
const FAULT_EP_CAP_IDX: u64 = 2;
const VSPACE_CAP_IDX: u64 = 3;
const REPLY_CAP_IDX: u64 = 4;
const MONITOR_EP_CAP_IDX: u64 = 5;
const TCB_CAP_IDX: u64 = 6;
const SMC_CAP_IDX: u64 = 7;

const BASE_OUTPUT_NOTIFICATION_CAP: u64 = 10;
const BASE_OUTPUT_ENDPOINT_CAP: u64 = BASE_OUTPUT_NOTIFICATION_CAP + 64;
const BASE_IRQ_CAP: u64 = BASE_OUTPUT_ENDPOINT_CAP + 64;
const BASE_PD_TCB_CAP: u64 = BASE_IRQ_CAP + 64;
const BASE_VM_TCB_CAP: u64 = BASE_PD_TCB_CAP + 64;
const BASE_VCPU_CAP: u64 = BASE_VM_TCB_CAP + 64;

const MAX_SYSTEM_INVOCATION_SIZE: u64 = util::mb(128);

const PD_CAP_SIZE: u64 = 512;
const PD_CAP_BITS: u64 = PD_CAP_SIZE.ilog2() as u64;
const PD_SCHEDCONTEXT_SIZE: u64 = 1 << 8;

const SLOT_BITS: u64 = 5;
const SLOT_SIZE: u64 = 1 << SLOT_BITS;

// @billn work out what do these do
// const INIT_NULL_CAP_ADDRESS: u64 = 0;
// const INIT_TCB_CAP_ADDRESS: u64 = 1;
// const INIT_CNODE_CAP_ADDRESS: u64 = 2;
// const INIT_VSPACE_CAP_ADDRESS: u64 = 3;
// const IRQ_CONTROL_CAP_ADDRESS: u64 = 4; // Singleton
// const INIT_ASID_POOL_CAP_ADDRESS: u64 = 6;
// const SMC_CAP_ADDRESS: u64 = 15;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct CapDLSpec {
    /// Whatever you do, DO NOT SORT! DO NOT SORT! DO NOT SORT!!!!!
    /// Because object IDs are index into the vectors
    pub objects: Vec<NamedObject>,
    pub irqs: Vec<IrqEntry>,
    pub asid_slots: Vec<AsidSlotEntry>,
    pub root_objects: Range<ObjectId>,
    pub untyped_covers: Vec<UntypedCover>,
}

impl CapDLSpec {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            irqs: Vec::new(),
            asid_slots: Vec::new(),
            root_objects: Range { start: 0, end: 0 },
            untyped_covers: Vec::new(),
        }
    }

    pub fn add_root_object(&mut self, obj: NamedObject) -> ObjectId {
        self.objects.push(obj);
        self.root_objects.end += 1;
        self.root_objects.end - 1
    }

    pub fn get_root_object_mut(&mut self, obj_id: ObjectId) -> Option<&mut NamedObject> {
        if obj_id < self.root_objects.end {
            Some(&mut self.objects[obj_id])
        } else {
            None
        }
    }

    pub fn get_root_object(&self, obj_id: ObjectId) -> Option<&NamedObject> {
        if obj_id < self.root_objects.end {
            Some(&self.objects[obj_id])
        } else {
            None
        }
    }

    /// Add the details of the given ELF into the given CapDL spec while inferring as much information
    /// as possible. These are the objects that will be created:
    /// -> TCB: PC and IPC buffer vaddr set. VSpace and IPC buffer frame caps bound.
    /// -> VSpace: all ELF loadable pages and IPC buffer mapped in.
    /// Returns the object ID of the TCB
    ///
    pub fn add_elf_to_spec(&mut self, pd_name: &str, elf: &ElfFile) -> Result<ObjectId, String> {
        // We assumes that ELFs and PDs have a one-to-one relationship. So for each ELF we create a VSpace.
        let vspace_obj_id = X86_64::create_vspace(self, pd_name); // @billn make arch agnostic
        let vspace_cap = Cap::PageTable(cap::PageTable {
            object: vspace_obj_id,
        });

        // For each loadable segment in the ELF, map it into the address space of this PD.
        let mut frame_sequence = 0; // For object naming purpose only.
        for segment in elf.loadable_segments() {
            if segment.data.len() == 0 {
                continue;
            }

            let seg_base_vaddr = segment.virt_addr;
            let seg_file_off = segment.p_offset;
            let seg_file_size: u64 = segment.p_filesz;
            let seg_mem_size: u64 = segment.mem_size();

            let page_size = PageSize::Small;
            let page_size_bytes = page_size as u64;

            // Starts from the page boundary
            let mut cur_vaddr = round_down(seg_base_vaddr, page_size_bytes);
            while cur_vaddr < seg_base_vaddr + seg_mem_size {
                let mut frame_fill = FrameInit::Fill(Fill {
                    entries: [].to_vec(),
                });

                // Now compute the ELF file offset to fill in this page.
                let mut dest_offset = 0;
                if cur_vaddr < seg_base_vaddr {
                    // Take care of case where the ELF segment is not aligned on page boundary:
                    //     |   ELF    |   ELF    |   ELF    |
                    // |   Page   |   Page   |   Page   |
                    //  <->
                    dest_offset = seg_base_vaddr - cur_vaddr;
                }

                let target_vaddr_start = cur_vaddr + dest_offset;
                let section_offset = target_vaddr_start - seg_base_vaddr;
                if section_offset < seg_file_size {
                    // Have data to load
                    let len_to_cpy = min(
                        page_size_bytes - dest_offset,
                        seg_file_size - section_offset,
                    );
                    let src_off = seg_file_off + section_offset;
                    match &mut frame_fill {
                        FrameInit::Fill(fill) => {
                            fill.entries.push(FillEntry {
                                range: Range {
                                    start: dest_offset as usize,
                                    end: (dest_offset + len_to_cpy) as usize,
                                },
                                content: FillEntryContent::Data(FileContentRange {
                                    file: elf.path.to_string_lossy().into_owned(),
                                    file_offset: src_off as usize,
                                }),
                            });
                        }
                    }
                }

                // Create the frame object, cap to the object, add it to the spec and map it in.
                let frame_obj_id = capdl_util_make_frame_obj(
                    self,
                    frame_fill,
                    &format!("{}_elf_{}", pd_name, frame_sequence),
                    None,
                    12, // @billn fix use ObjectType::fixed_size_bits
                );
                let frame_cap = capdl_util_make_frame_cap(
                    frame_obj_id,
                    segment.is_readable(),
                    segment.is_writable(),
                    segment.is_executable(),
                    true,
                );

                // @billn make arch agnostic
                match memory::X86_64::map_page(
                    self,
                    pd_name,
                    vspace_obj_id,
                    frame_cap,
                    page_size,
                    cur_vaddr,
                ) {
                    Ok(_) => {
                        frame_sequence += 1;
                        cur_vaddr += page_size_bytes;
                    }
                    Err(map_err_reason) => {
                        return Err(format!(
                            "add_elf_to_spec(): failed to map segment page to ELF because: {}",
                            map_err_reason
                        ))
                    }
                };
            }
        }

        // Create and map the IPC buffer for this ELF
        let ipcbuf_frame_obj_id = capdl_util_make_frame_obj(
            self,
            FrameInit::Fill(Fill {
                entries: [].to_vec(),
            }),
            &format!("{}_ipcbuf", pd_name),
            None,
            12, // @billn fix
        );
        let ipcbuf_frame_cap =
            capdl_util_make_frame_cap(ipcbuf_frame_obj_id, true, true, false, true);
        // We need to clone the IPC buf cap because in addition to mapping the frame into the VSpace, we need to bind
        // this frame to the TCB as well.
        let ipcbuf_frame_cap_for_tcb = ipcbuf_frame_cap.clone();
        let ipcbuf_vaddr = elf
            .find_symbol(SYMBOL_IPC_BUFFER)
            .unwrap_or_else(|_| panic!("Could not find {}", SYMBOL_IPC_BUFFER))
            .0;
        match memory::X86_64::map_page(
            self,
            pd_name,
            vspace_obj_id,
            ipcbuf_frame_cap,
            PageSize::Small,
            ipcbuf_vaddr,
        ) {
            Ok(_) => {}
            Err(map_err_reason) => {
                return Err(format!(
                    "build_capdl_spec(): failed to map ipc buffer frame to monitor because: {}",
                    map_err_reason
                ))
            }
        };

        let tcb_name = format!("tcb_{}", pd_name);
        let entry_point = elf.entry;

        let tcb_extra_info = object::TcbExtraInfo {
            ipc_buffer_addr: ipcbuf_vaddr,
            affinity: 0, // @billn fix for smp
            prio: 0,
            max_prio: 0,
            resume: false,
            ip: entry_point,
            sp: 0,
            gprs: Vec::new(),
            master_fault_ep: None,
        };

        let tcb_inner_obj = object::Tcb {
            // Bind the VSpace into the TCB
            slots: [
                (TCB_SLOT_VSPACE as usize, vspace_cap),
                (TCB_SLOT_IPC_BUFFER as usize, ipcbuf_frame_cap_for_tcb),
            ]
            .to_vec(),
            extra: tcb_extra_info,
        };

        let tcb_obj = NamedObject {
            name: tcb_name,
            object: Object::Tcb(tcb_inner_obj),
        };

        Ok(self.add_root_object(tcb_obj))
    }
}

/// Build a CapDL Spec according to the System Description File.
pub fn build_capdl_spec(
    kernel_config: &Config,
    capdl_initialiser_elf_path: &PathBuf,
    monitor_elf: &ElfFile,
    pd_elf_files: &Vec<ElfFile>,
    system: &SystemDescription,
) -> Result<CapDLSpec, String> {
    let mut spec = CapDLSpec::new();

    // *********************************
    // Step 1. Create the monitor's spec.
    // *********************************

    // Parse ELF, create VSpace, map in all ELF loadable frames and IPC buffer, and create TCB.
    let monitor_tcb_obj_id = spec.add_elf_to_spec("monitor", monitor_elf)?; // @billn check error

    // Create monitor fault endpoint object + cap
    let mon_fault_ep_obj_id = capdl_util_make_endpoint_obj(&mut spec, "monitor");
    let mon_fault_ep_cap = capdl_util_make_endpoint_cap(mon_fault_ep_obj_id, 0);

    // Create monitor reply object object + cap
    let mon_reply_obj_id = capdl_util_make_reply_obj(&mut spec, "monitor");
    let mon_reply_cap = capdl_util_make_reply_cap(mon_reply_obj_id);

    // Create monitor scheduling context object + cap
    // @billn work out where these magic numbers come from and fix size bits
    let mon_sc_obj_id = capdl_util_make_sc_obj(&mut spec, "monitor", 7, 100, 100, 0);
    let mon_sc_cap = capdl_util_make_sc_cap(mon_sc_obj_id);

    // Create monitor CSpace and pre-insert the fault EP and reply caps into the correct slots in CSpace.
    let mon_cnode_obj_id = capdl_util_make_cnode_obj(
        &mut spec,
        "monitor",
        PD_CAP_BITS as usize,
        [
            (FAULT_EP_CAP_IDX as usize, mon_fault_ep_cap),
            (REPLY_CAP_IDX as usize, mon_reply_cap),
        ]
        .to_vec(),
    );
    // @billn understand???: guard_size: kernel_config.cap_address_bits - PD_CAP_BITS,
    let mon_cnode_cap = capdl_util_make_cnode_cap(mon_cnode_obj_id, 0, 55);

    // At this point, all of the required objects for the monitor have been created and it caps inserted into
    // the correct slot in the CSpace. We need to bind those objects into the TCB for the monitor to use them.
    // In addition, `add_elf_to_spec()` doesn't fill most the details in the TCB.
    // Now fill them in: stack ptr, priority, ipc buf vaddr, etc.
    {
        let monitor_tcb_wrapper_obj = spec.get_root_object_mut(monitor_tcb_obj_id).unwrap();
        if let Object::Tcb(monitor_tcb) = &mut monitor_tcb_wrapper_obj.object {
            // Special case, monitor have its stack statically allocated.
            monitor_tcb.extra.sp = monitor_elf.find_symbol("_stack").unwrap().0;
            monitor_tcb.extra.master_fault_ep = None;
            // Monitor must run at the highest priority
            monitor_tcb.extra.prio = u8::MAX - 1;
            monitor_tcb.extra.max_prio = u8::MAX - 1;
            monitor_tcb.extra.resume = true;

            monitor_tcb
                .slots
                .push((TCB_SLOT_CSPACE as usize, mon_cnode_cap));

            monitor_tcb.slots.push((TCB_SLOT_SC as usize, mon_sc_cap));
        } else {
            unreachable!("internal bug: build_capdl_spec() got a non TCB object ID when trying to set TCB parameters for the monitor.");
        }
    }

    // *********************************
    // Step 2. Create the memory regions' spec. Result is a hashmap keyed on MR name, value is Vec of frame caps
    // *********************************

    // *********************************
    // Step 3. Create the PDs' spec
    // *********************************
    // for (i, pd) in system.protection_domains.iter().enumerate() {
    //     let elf = &pd_elf_files[i];
    //     let pd_tcb_obj_id = spec.add_elf_to_spec(&pd.name, elf)?; // @billn check error

    //     // Same as the monitor, we must pull in extra details for the TCB from the SDF.
    // }

    // *********************************
    // Step 4. Serialise the spec to JSON
    // *********************************

    // *********************************
    // Step 4. Embed the serialised spec to the CapDL loader
    // *********************************

    Ok(spec)
}
