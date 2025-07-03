//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//
use core::ops::Range;

use std::cmp::min;

use serde::{Deserialize, Serialize};

use crate::{
    capdl::{memory::{self, ArchMethods, X86_64}, spec::{
        cap, object::{self}, AsidSlotEntry, Cap, CapTableEntry, FileContentRange, Fill, FillEntry, FrameInit, IrqEntry, NamedObject, Object, ObjectId, Rights, UntypedCover
    }},
    elf::ElfFile,
    sel4::PageSize,
    util::round_down,
};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct CapDLSpec {
    pub objects: Vec<NamedObject>,
    pub irqs: Vec<IrqEntry>,
    pub asid_slots: Vec<AsidSlotEntry>,
    pub root_objects: Range<ObjectId>,
    pub untyped_covers: Vec<UntypedCover>,
}

// impl<'a, N: ObjectName, D: Content, M: GetEmbeddedFrame> CapDLSpec<'a, N, D, M> {
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
}

/// Add the details of the given ELF into the given CapDL spec while inferring as much information
/// as possible. These are the objects that will be created:
/// -> TCB: pc and vspace set
/// -> VSpace: all ELF loadable pages mapped in.
/// Returns the object ID of the TCB
/// 
pub fn add_elf_to_spec(
    spec: &mut CapDLSpec,
    pd_name: &str,
    elf: &ElfFile
) -> Result<ObjectId, String> {
    let vspace_obj = X86_64::vspace(pd_name);
    let vspace_id = spec.add_root_object(vspace_obj);
    let vspace_cap = Cap::PageTable(cap::PageTable {
        object: vspace_id,
    });

    // For each loadable segment in the ELF, map it into the address space of this PD.
    let mut frame_number = 0; // For object naming purpose only.
    for segment in elf.loadable_segments() {
        if segment.data.len() == 0 {
            continue;
        }

        let seg_base_vaddr = segment.virt_addr;
        let seg_file_off = segment.p_offset;
        let seg_size: u64 = segment.p_filesz;

        let page_size = PageSize::Small;
        let page_size_bytes = page_size as u64;

        // Starts from the page boundary
        let mut cur_vaddr = round_down(seg_base_vaddr, page_size_bytes);
        while cur_vaddr < seg_base_vaddr + seg_size {
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
            if section_offset < seg_size {
                // Have data to load
                let len_to_cpy = min(page_size_bytes - dest_offset, seg_size - section_offset);
                let src_off = seg_file_off + section_offset;
                match &mut frame_fill {
                    FrameInit::Fill(fill) => {
                        fill.entries.push(FillEntry {
                            range: Range {
                                start: dest_offset as usize,
                                end: (dest_offset + len_to_cpy) as usize,
                            },
                            content: FileContentRange {
                                file: elf.path.to_string_lossy().into_owned(),
                                file_offset: src_off as usize,
                                file_length: len_to_cpy as usize,
                            },
                        });
                    }
                }
            }

            // Create the frame object, cap to the object, add it to the spec and map it in.
            let frame_inner_obj = Object::Frame(object::Frame {
                size_bits: 12, // @billn fix use ObjectType::fixed_size_bits
                paddr: None,
                init: frame_fill,
            });
            let frame_obj = NamedObject {
                name: format!("frame_{}_{}", pd_name, frame_number),
                object: frame_inner_obj,
            };
            let frame_obj_id = spec.add_root_object(frame_obj);
            let frame_cap = Cap::Frame(cap::Frame {
                object: frame_obj_id,
                rights: Rights {
                    read: segment.is_readable(),
                    write: segment.is_writable(),
                    grant: segment.is_executable(),
                    grant_reply: false,
                },
                cached: true,
            });

            // @billn print error detail
            memory::X86_64::map_page(spec, pd_name, vspace_id, frame_cap, page_size, cur_vaddr)?;

            frame_number += 1;
            cur_vaddr += page_size_bytes;
        }
    }

    let tcb_name = format!("tcb_{}", pd_name);
    let entry_point = elf.entry;

    let tcb_extra_info = object::TcbExtraInfo {
        ipc_buffer_addr: 0,
        affinity: 0,
        prio: 0,
        max_prio: 0,
        resume: false,
        ip: entry_point,
        sp: 0,
        gprs: Vec::new(),
        master_fault_ep: None,
    };

    let tcb_inner_obj = object::Tcb {
        slots: [(0, vspace_cap)].to_vec(),
        extra: tcb_extra_info,
    };

    let tcb_obj = NamedObject {
        name: tcb_name,
        object: Object::Tcb(tcb_inner_obj),
    };

    Ok(spec.add_root_object(tcb_obj))
}
