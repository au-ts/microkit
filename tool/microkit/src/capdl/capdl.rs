//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//
use core::ops::Range;

use std::{
    cell::RefCell,
    cmp::{min, Ordering},
    collections::HashMap,
    rc::Rc,
    u8,
};

use serde::Serialize;

use crate::{
    capdl::{
        memory::{self, ArchMethods, X86_64},
        spec::{
            cap,
            object::{self},
            AsidSlotEntry, BytesContent, Cap, CapTableEntry, Fill, FillEntry, FillEntryContent,
            FrameInit, IrqEntry, NamedObject, Object, ObjectId, UntypedCover,
        },
        util::*,
    },
    elf::ElfFile,
    sdf::{self, SysMapPerms, SystemDescription},
    sel4::{Config, PageSize},
    util::{pd_write_symbols, round_down},
};

// Corresponds to the IPC buffer symbol in libmicrokit and the monitor
const SYMBOL_IPC_BUFFER: &str = "__sel4_ipc_buffer_obj";

// @billn figure out where these are used
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

#[derive(Serialize, Clone, Eq, PartialEq)]
pub struct CapDLSpec {
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
    pub fn add_elf_to_spec(
        &mut self,
        sel4_config: &Config,
        pd_name: &str,
        elf: Rc<RefCell<ElfFile>>,
    ) -> Result<ObjectId, String> {
        // We assumes that ELFs and PDs have a one-to-one relationship. So for each ELF we create a VSpace.
        let vspace_obj_id = X86_64::create_vspace(self, pd_name); // @billn make arch agnostic
        let vspace_cap = Cap::PageTable(cap::PageTable {
            object: vspace_obj_id,
        });

        // For each loadable segment in the ELF, map it into the address space of this PD.
        let mut frame_sequence = 0; // For object naming purpose only.
        for segment in elf.borrow().loadable_segments().iter() {
            if segment.data.len() == 0 {
                continue;
            }

            let seg_base_vaddr = segment.virt_addr;
            let seg_file_size: u64 = segment.p_filesz;
            let seg_mem_size: u64 = segment.mem_size();

            let page_size = PageSize::Small;
            let page_size_bytes = page_size as u64;

            // Create and map all frames for this segment.
            let mut cur_vaddr = round_down(seg_base_vaddr, page_size_bytes);
            while cur_vaddr < seg_base_vaddr + seg_mem_size {
                let mut frame_init_maybe: Option<FrameInit> = None;

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
                    // We have data to load
                    let len_to_cpy = min(
                        page_size_bytes - dest_offset,
                        seg_file_size - section_offset,
                    );

                    frame_init_maybe = Some(FrameInit::Fill(Fill {
                        entries: [FillEntry {
                            range: Range {
                                start: dest_offset as usize,
                                end: (dest_offset + len_to_cpy) as usize,
                            },
                            content: FillEntryContent::Data(BytesContent {
                                bytes: segment.data[section_offset as usize
                                    ..((section_offset + len_to_cpy) as usize)]
                                    .to_vec(),
                            }),
                        }]
                        .to_vec(),
                    }));
                }

                let frame_init = match frame_init_maybe {
                    Some(actual_frame_init) => actual_frame_init,
                    None => FrameInit::Fill(Fill {
                        entries: [].to_vec(),
                    }),
                };
                // Create the frame object, cap to the object, add it to the spec and map it in.
                let frame_obj_id = capdl_util_make_frame_obj(
                    self,
                    frame_init,
                    &format!("elf_{}_{}", pd_name, frame_sequence),
                    None,
                    PageSize::Small.fixed_size_bits(sel4_config) as usize,
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
            // Must be consistent with the granule bits used in spec serialisation
            PageSize::Small.fixed_size_bits(sel4_config) as usize,
        );
        let ipcbuf_frame_cap =
            capdl_util_make_frame_cap(ipcbuf_frame_obj_id, true, true, false, true);
        // We need to clone the IPC buf cap because in addition to mapping the frame into the VSpace, we need to bind
        // this frame to the TCB as well.
        let ipcbuf_frame_cap_for_tcb = ipcbuf_frame_cap.clone();
        let ipcbuf_vaddr = elf
            .borrow()
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
        let entry_point = elf.borrow().entry;

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
    monitor_elf: Rc<RefCell<ElfFile>>,
    pd_elf_files: &mut Vec<Rc<RefCell<ElfFile>>>,
    system: &SystemDescription,
) -> Result<CapDLSpec, String> {
    let mut spec = CapDLSpec::new();

    // @billn revisit: does every caps need grant rights? Apart from executable data needing grant

    // *********************************
    // Step 1. Create the monitor's spec.
    // *********************************

    // Parse ELF, create VSpace, map in all ELF loadable frames and IPC buffer, and create TCB.
    let monitor_tcb_obj_id = spec
        .add_elf_to_spec(kernel_config, "monitor", monitor_elf.clone())
        .unwrap(); // @billn check error

    // Create monitor fault endpoint object + cap
    let mon_fault_ep_obj_id = capdl_util_make_endpoint_obj(&mut spec, "monitor");
    let mon_fault_ep_cap = capdl_util_make_endpoint_cap(mon_fault_ep_obj_id, true, true, true, 0);

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
    if let Object::Tcb(monitor_tcb) =
        &mut spec.get_root_object_mut(monitor_tcb_obj_id).unwrap().object
    {
        // Special case, monitor have its stack statically allocated.
        monitor_tcb.extra.sp = monitor_elf.borrow().find_symbol("_stack").unwrap().0;
        // Monitor must run at the highest priority
        monitor_tcb.extra.prio = u8::MAX;
        monitor_tcb.extra.max_prio = u8::MAX;
        monitor_tcb.extra.resume = true;

        monitor_tcb
            .slots
            .push((TCB_SLOT_CSPACE as usize, mon_cnode_cap));

        monitor_tcb.slots.push((TCB_SLOT_SC as usize, mon_sc_cap));
    } else {
        unreachable!("internal bug: build_capdl_spec() got a non TCB object ID when trying to set TCB parameters for the monitor.");
    }

    // *********************************
    // Step 2. Create the memory regions' spec. Result is a hashmap keyed on MR name, value is Vec of frame object IDs
    // *********************************
    let mut mr_to_frame_obj_ids: HashMap<&String, Vec<ObjectId>> = HashMap::new();
    let mut mr_to_page_size: HashMap<&String, PageSize> = HashMap::new();
    for mr in system.memory_regions.iter() {
        mr_to_frame_obj_ids.insert(&mr.name, [].to_vec());
        mr_to_page_size.insert(&mr.name, mr.page_size);
        let frame_size_bits = mr.page_size.fixed_size_bits(kernel_config);

        for frame_sequence in 0..mr.page_count {
            let paddr = match mr.phys_addr {
                Some(base_paddr) => {
                    Some((base_paddr + (frame_sequence * mr.page_size_bytes())) as usize)
                }
                None => None,
            };
            mr_to_frame_obj_ids
                .get_mut(&mr.name)
                .unwrap()
                .push(capdl_util_make_frame_obj(
                    &mut spec,
                    FrameInit::Fill(Fill {
                        entries: [].to_vec(),
                    }),
                    &format!("mr_{}_{}", mr.name, frame_sequence),
                    paddr,
                    frame_size_bits as usize,
                ));
        }
    }

    // *********************************
    // Step 3. Create the PDs' spec
    // *********************************
    // // Before we do anything though, we write all ELF symbols we need into every PD's ELF data structure.
    // // So that spec.add_elf_to_spec() will just add the correct data into the spec for us and we don't have to
    // // touch the frames data again at a later step.
    // let pd_setvar_values: Vec<Vec<u64>> = system
    //     .protection_domains
    //     .iter()
    //     .map(|pd| {
    //         pd.setvars
    //             .iter()
    //             .map(|setvar| match &setvar.kind {
    //                 sdf::SysSetVarKind::Size { mr } => {
    //                     system
    //                         .memory_regions
    //                         .iter()
    //                         .find(|m| m.name == *mr)
    //                         .unwrap()
    //                         .size
    //                 }
    //                 sdf::SysSetVarKind::Vaddr { address } => *address,
    //                 sdf::SysSetVarKind::Paddr { region } => {
    //                     let mr = system
    //                         .memory_regions
    //                         .iter()
    //                         .find(|mr| mr.name == *region)
    //                         .unwrap_or_else(|| panic!("Cannot find region: {}", region));

    //                     mr_pages[mr][0].phys_addr
    //                 }
    //             })
    //             .collect()
    //     })
    //     .collect();

    // pd_write_symbols(&system.protection_domains, &system.channels, pd_elf_files, pd_setvar_values);

    for (pd_id, pd) in system.protection_domains.iter().enumerate() {
        let mut caps_to_bind_to_tcb: Vec<CapTableEntry> = Vec::new();
        let mut caps_to_insert_to_cspace: Vec<CapTableEntry> = Vec::new();

        // Step 3-1: Create TCB and VSpace with all ELF loadable frames mapped in.
        let elf = pd_elf_files[pd_id].clone();
        let pd_tcb_obj_id = spec.add_elf_to_spec(kernel_config, &pd.name, elf).unwrap();
        let pd_vspace_obj_id = capdl_util_get_vspace_id_from_tcb_id(&spec, pd_tcb_obj_id);

        // In the benchmark configuration, we allow PDs to access their own TCB.
        // This is necessary for accessing kernel's benchmark API.
        if kernel_config.benchmark {
            caps_to_insert_to_cspace.push((
                TCB_CAP_IDX as usize,
                Cap::Tcb(cap::Tcb {
                    object: pd_tcb_obj_id,
                }),
            ));
        }

        // Step 3-2: Map in all Memory Regions
        for map in pd.maps.iter() {
            let cur_vaddr = map.vaddr;
            let page_size = *mr_to_page_size.get(&map.mr).unwrap();
            let read = map.perms & SysMapPerms::Read as u8 != 0;
            let write = map.perms & SysMapPerms::Write as u8 != 0;
            let execute = map.perms & SysMapPerms::Execute as u8 != 0;
            let cached = map.cached;
            for frame_obj_id in mr_to_frame_obj_ids.get(&map.mr).unwrap() {
                // Make a cap for this frame.
                let frame_cap =
                    capdl_util_make_frame_cap(*frame_obj_id, read, write, execute, cached);
                // Map it into this PD address space. @billn make arch agnositc
                memory::X86_64::map_page(
                    &mut spec,
                    &pd.name,
                    pd_vspace_obj_id,
                    frame_cap,
                    page_size,
                    cur_vaddr,
                )
                .unwrap();
            }
        }

        // Step 3-3: Create and map in the stack (bottom up)
        let mut cur_stack_vaddr = kernel_config.pd_stack_bottom(pd.stack_size);
        let num_stack_frames = pd.stack_size / PageSize::Small as u64;
        for stack_frame_seq in 0..num_stack_frames {
            let stack_frame_obj_id = capdl_util_make_frame_obj(
                &mut spec,
                FrameInit::Fill(Fill {
                    entries: [].to_vec(),
                }),
                &format!("{}_stack_{}", pd.name, stack_frame_seq),
                None,
                PageSize::Small.fixed_size_bits(kernel_config) as usize,
            );
            let stack_frame_cap =
                capdl_util_make_frame_cap(stack_frame_obj_id, true, true, false, true);
            memory::X86_64::map_page(
                &mut spec,
                &pd.name,
                pd_vspace_obj_id,
                stack_frame_cap,
                PageSize::Small,
                cur_stack_vaddr,
            )
            .unwrap();
            cur_stack_vaddr += PageSize::Small as u64;
        }

        // Step 3-4 Create Scheduling Context
        // @billn work out where these magic numbers come from and fix size bits
        let pd_sc_obj_id = capdl_util_make_sc_obj(&mut spec, &pd.name, 7, pd.period, pd.budget, 0);
        let pd_sc_cap = capdl_util_make_sc_cap(pd_sc_obj_id);
        caps_to_bind_to_tcb.push((TCB_SLOT_SC as usize, pd_sc_cap));

        // Step 3-5 Create fault Endpoint cap to monitor
        let pd_fault_ep_cap =
            capdl_util_make_endpoint_cap(mon_fault_ep_obj_id, true, true, true, pd_id as u64);
        let pd_fault_ep_cap_clone = pd_fault_ep_cap.clone();
        caps_to_insert_to_cspace.push((FAULT_EP_CAP_IDX as usize, pd_fault_ep_cap));
        caps_to_bind_to_tcb.push((TCB_SLOT_FAULT_EP as usize, pd_fault_ep_cap_clone));

        // Step 3-6 Create spec and caps to IRQs
        let mut irq_caps: Vec<Cap> = Vec::new();
        for irq in pd.irqs.iter() {}

        // Create CSpace and add all caps that the PD code and libmicrokit need to access.
        let pd_cnode_obj_id = capdl_util_make_cnode_obj(
            &mut spec,
            &pd.name,
            PD_CAP_BITS as usize,
            caps_to_insert_to_cspace,
        );
        // @billn understand???: guard_size: kernel_config.cap_address_bits - PD_CAP_BITS,
        let pd_cnode_cap = capdl_util_make_cnode_cap(pd_cnode_obj_id, 0, 55);
        caps_to_bind_to_tcb.push((TCB_SLOT_CSPACE as usize, pd_cnode_cap));

        // Set the TCB parameters and all the various caps that we need to bind to this TCB.
        if let Object::Tcb(pc_tcb) = &mut spec.get_root_object_mut(pd_tcb_obj_id).unwrap().object {
            pc_tcb.extra.sp = kernel_config.pd_stack_top();
            pc_tcb.extra.master_fault_ep = Some(FAULT_EP_CAP_IDX);
            pc_tcb.extra.prio = pd.priority;
            pc_tcb.extra.max_prio = pd.priority; // Prevent priority escalation.
            pc_tcb.extra.resume = true;

            pc_tcb.slots.extend(caps_to_bind_to_tcb);
        } else {
            unreachable!("internal bug: build_capdl_spec() got a non TCB object ID when trying to set TCB parameters for the monitor.");
        }
    }

    // *********************************
    // Step 4. Write ELF symbols in the monitor and PDs.
    // *********************************

    // *********************************
    // Step 5. Sort the root objects
    // *********************************
    // The CapDL loader expects objects with paddr to come first, then sorted by size so that the
    // allocation algorithm at run-time can run more efficiently.
    // Capabilities to objects in CapDL are referenced by the object's index in the root objects
    // vector. Since sorting the objects will shuffle them, we need to:
    // 1. Record all root objects name + original index.
    // 2. Sort paddr first, size bits descending and break tie alphabetically.
    // 3. Record all of the root objects new index.
    // 4. Recurse through every cap, for any cap bearing the original object ID, write the new object ID.

    // Step 4-1
    let mut obj_name_to_old_id: HashMap<String, ObjectId> = HashMap::new();
    for (id, obj) in spec.objects.iter().enumerate() {
        obj_name_to_old_id.insert(obj.name.clone(), id);
    }

    // Step 4-2
    spec.objects.sort_by(|a, b| {
        // Objects with paddrs come first.
        if a.object.paddr().is_none() && a.object.paddr().is_some() {
            return Ordering::Less;
        } else if a.object.paddr().is_some() && a.object.paddr().is_none() {
            return Ordering::Greater;
        }

        // If both have paddrs and not equal, make the lower paddr comes first.
        if a.object.paddr().is_some() && b.object.paddr().is_some() {
            let a_paddr = a.object.paddr().unwrap();
            let b_paddr = b.object.paddr().unwrap();
            if a_paddr != b_paddr {
                return a_paddr.cmp(&b_paddr);
            }
        }
        // Both have no paddr or equal paddr, break tie by object size and name.

        let size_cmp = a
            .object
            .physical_size_bits(kernel_config)
            .cmp(&b.object.physical_size_bits(kernel_config))
            .reverse();
        if size_cmp == Ordering::Equal {
            let name_cmp = a.name.cmp(&b.name);
            if name_cmp == Ordering::Equal {
                // Make sure the sorting function implement a total order to comply with .sort_by()'s doc.
                unreachable!("internal bug: object names must be unique!");
            }
            name_cmp
        } else {
            size_cmp
        }
    });

    // Step 4-3
    let mut obj_old_id_to_new_id: HashMap<ObjectId, ObjectId> = HashMap::new();
    for (new_id, obj) in spec.objects.iter().enumerate() {
        obj_old_id_to_new_id.insert(*obj_name_to_old_id.get(&obj.name).unwrap(), new_id);
    }

    // Step 4-4
    for obj in spec.objects.iter_mut() {
        match obj.object.get_cap_entries_mut() {
            Some(caps) => {
                for cap in caps {
                    let old_id = cap.1.obj();
                    let new_id = obj_old_id_to_new_id.get(&old_id).unwrap();
                    cap.1.set_id(*new_id);
                }
            }
            None => continue,
        }
    }

    Ok(spec)
}
