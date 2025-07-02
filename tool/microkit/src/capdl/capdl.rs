//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//
use core::ops::Range;

use std::{cmp::min, collections::HashSet};

use serde::{Deserialize, Serialize};

use crate::{capdl::{page_collection::PageCollection, spec::{object, AsidSlotEntry, FileContentRange, Fill, FillEntry, FrameInit, IrqEntry, NamedObject, Object, ObjectId, UntypedCover}}, elf::ElfFile, sdf::SysMapPerms, sel4::PageSize, util::{round_down, round_up}};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct CapDLSpec {
    pub objects: HashSet<NamedObject>,
    pub irqs: HashSet<IrqEntry>,
    pub asid_slots: HashSet<AsidSlotEntry>,
    pub root_objects: Range<ObjectId>,
    pub untyped_covers: HashSet<UntypedCover>
}

// impl<'a, N: ObjectName, D: Content, M: GetEmbeddedFrame> CapDLSpec<'a, N, D, M> {
impl CapDLSpec {
    fn add_root_object(&mut self, obj: NamedObject) {
        self.objects.insert(obj);
        self.root_objects.end += 1;
    }

    // Create a CapDL spec from the given ELF file, infer as much information as possible.
    pub fn from_elf(
        name: &String,
        elf: &ElfFile,
        infer_tcb: bool,
    ) -> Result<Self, String> {
        let mut spec = Self{
            objects: HashSet::new(),
            irqs: HashSet::new(),
            asid_slots: HashSet::new(),
            root_objects: Range { start: 0, end: 0 },
            untyped_covers: HashSet::new(),
        };

        if infer_tcb {
            let tcb_name = format!("tcb_{}", name);
            let entry_point = elf.entry;

            println!("elf pc 0x{:x}", entry_point);

            let tcb_extra_info = object::TcbExtraInfo {
                ipc_buffer_addr: 0,
                affinity: 0,
                prio: 0,
                max_prio: 0,
                resume: false,
                ip: entry_point,
                sp: 0,
                gprs: Vec::new(),
                master_fault_ep: None
            };

            let tcb_obj = object::Tcb {
                slots: Vec::new(),
                extra: tcb_extra_info,
            };

            let named_tcb = NamedObject {
                name: tcb_name,
                object: Object::Tcb(tcb_obj)
            };
            spec.add_root_object(named_tcb);
        }

        // Create paging structures and mapping spec for all loadable segments in the ELF
        let mut pages_collection = PageCollection::new();
        for segment in elf.loadable_segments() {
            if segment.data.len() == 0 {
                continue
            }

            let seg_base_vaddr = segment.virt_addr;
            let seg_file_off = segment.p_offset;
            let seg_size: u64 = segment.p_filesz;

            let page_size = PageSize::Small;
            let page_size_bytes = page_size as u64;

            let mut perms = 0;
            if segment.is_readable() {
                perms |= SysMapPerms::Read as u8;
            }
            if segment.is_writable() {
                perms |= SysMapPerms::Write as u8;
            }
            if segment.is_executable() {
                perms |= SysMapPerms::Execute as u8;
            }

            // Starts from the page boundary
            let mut cur_vaddr = round_down(seg_base_vaddr, page_size_bytes);
            while cur_vaddr < seg_base_vaddr + seg_size {
                let mut frame_fill = FrameInit::Fill(Fill {entries: [].to_vec()});

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

                pages_collection.add_page(cur_vaddr, perms, page_size, frame_fill);

                cur_vaddr += page_size_bytes;
            }
        }
        println!("{:#?}", pages_collection);
        Ok(spec)
    }

    pub fn merge(&mut self, from_spec: Self) {
        let num_our_objs = self.objects.len();
        let num_oth_objs = from_spec.objects.len();
        self.objects.extend(from_spec.objects);
        if self.objects.len() == num_our_objs + num_oth_objs {
            println!("WARNING, some objects were dropped during CapDL spec merge.\n");
        }

        let num_our_irqs = self.irqs.len();
        let num_oth_irqs = from_spec.irqs.len();
        self.irqs.extend(from_spec.irqs);
        if self.irqs.len() == num_our_irqs + num_oth_irqs {
            println!("WARNING, some irqs were dropped during CapDL spec merge.\n");
        }

        let num_our_asid_slots = self.asid_slots.len();
        let num_oth_asid_slots = from_spec.asid_slots.len();
        self.asid_slots.extend(from_spec.asid_slots);
        if self.asid_slots.len() == num_our_asid_slots + num_oth_asid_slots {
            println!("WARNING, some ASID slots were dropped during CapDL spec merge.\n");
        }

        self.untyped_covers.extend(from_spec.untyped_covers);
        self.root_objects.end = from_spec.root_objects.end - from_spec.root_objects.start;
    }
}
