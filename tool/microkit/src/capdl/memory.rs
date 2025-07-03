//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//
use crate::{
    capdl::{
        spec::{cap, object, Cap, NamedObject, Object, ObjectId},
        CapDLSpec,
    },
    sel4::PageSize,
};

pub trait ArchMethods {
    fn vspace(pd_name: &str) -> NamedObject;
    fn map_page(
        spec: &mut CapDLSpec,
        pd_name: &str,
        vspace_id: ObjectId,
        frame_cap: Cap,
        frame_size: PageSize,
        vaddr: u64,
    ) -> Result<(), String>;
}

pub struct X86_64 {}

fn map_intermediary_level_helper(
    spec: &mut CapDLSpec,
    pd_name: &str,
    next_level_name_prefix: &str,
    cur_level_obj_id: ObjectId,
    cur_level: u8,
    cur_level_slot: u64,
) -> ObjectId {
    match spec.get_root_object_mut(cur_level_obj_id) {
        Some(vspace_obj) => {
            match &mut vspace_obj.object {
                Object::PageTable(page_table_object) => {
                    match page_table_object
                        .slots
                        .iter()
                        .find(|cte| cte.0 == cur_level_slot as usize)
                    {
                        Some(cte_unwrapped) => {
                            // Next level object already created, nothing to do
                            return cte_unwrapped.1.obj();
                        }
                        None => {
                            // We need to create the next level paging structure, get out of this scope for now
                            // so we don't get a double mutable borrow of spec when we need to insert the next level object
                        }
                    }
                }
                _ => todo!(),
            }
        }
        None => {
            eprintln!("map_intermediary_level_helper(): object ID {} not found when trying to map at level #{}", cur_level_obj_id, cur_level);
        }
    }

    // Next level object not already created, create it.
    let next_level_inner_obj = object::PageTable {
        is_root: false, // because the VSpace has already been created separately
        level: Some(cur_level + 1),
        slots: [].to_vec(),
    };
    let next_level_object = NamedObject {
        name: format!("{}_{}_{}", next_level_name_prefix, pd_name, cur_level_slot),
        object: Object::PageTable(next_level_inner_obj),
    };
    let next_level_obj_id = spec.add_root_object(next_level_object);

    // @billn revisit, looks a bit strange, refactor to insert_frame_cap_into_level()
    match spec.get_root_object_mut(cur_level_obj_id) {
        Some(vspace_obj) => {
            match &mut vspace_obj.object {
                Object::PageTable(page_table_object) => {
                    match page_table_object
                        .slots
                        .iter()
                        .find(|cte| cte.0 == cur_level_slot as usize)
                    {
                        Some(_) => {}
                        None => {
                            // Then create a Cap to it and insert it into the required slot
                            let next_level_cap = Cap::PageTable(cap::PageTable {
                                object: next_level_obj_id,
                            });
                            page_table_object
                                .slots
                                .push((cur_level_slot as usize, next_level_cap));
                        }
                    }
                }
                _ => todo!(),
            }
        }
        None => todo!(),
    }

    next_level_obj_id
}

fn insert_frame_cap_into_level(
    spec: &mut CapDLSpec,
    last_level_paging_obj_id: ObjectId,
    last_level_slot: u64,
    frame_cap: Cap,
) {
    match spec.get_root_object_mut(last_level_paging_obj_id) {
        Some(last_level_paging_obj) => match &mut last_level_paging_obj.object {
            Object::PageTable(last_level_paging_inner_obj) => {
                last_level_paging_inner_obj
                    .slots
                    .push((last_level_slot as usize, frame_cap));
            }
            _ => todo!(),
        },
        None => todo!(),
    }
}

impl ArchMethods for X86_64 {
    fn vspace(pd_name: &str) -> NamedObject {
        NamedObject {
            name: format!("pml4_{}", pd_name),
            object: Object::PageTable(object::PageTable {
                is_root: true,
                level: Some(0),
                slots: [].to_vec(),
            }),
        }
    }

    fn map_page(
        spec: &mut CapDLSpec,
        pd_name: &str,
        vspace_id: ObjectId,
        frame_cap: Cap,
        frame_size: PageSize,
        vaddr: u64,
    ) -> Result<(), String> {
        match &frame_cap {
            Cap::Frame(_) => {
                assert!(vaddr % frame_size as u64 == 0);

                // Get slot indexes for the 4 levels of the page table
                let pml4_slot = (vaddr >> (12 + 9 + 9 + 9)) & ((1 << 9) - 1);
                let pdpt_slot = (vaddr >> (12 + 9 + 9)) & ((1 << 9) - 1);
                let pd_slot = (vaddr >> (12 + 9)) & ((1 << 9) - 1);
                let pt_slot = (vaddr >> (12)) & ((1 << 9) - 1);

                // @billn handle huge page
                let pdpt_obj_id: ObjectId =
                    map_intermediary_level_helper(spec, pd_name, "pdpt", vspace_id, 0, pml4_slot);
                let pd_obj_id: ObjectId =
                    map_intermediary_level_helper(spec, pd_name, "pd", pdpt_obj_id, 1, pdpt_slot);
                let pt_obj_id: ObjectId =
                    map_intermediary_level_helper(spec, pd_name, "pt", pd_obj_id, 2, pd_slot);
                insert_frame_cap_into_level(spec, pt_obj_id, pt_slot, frame_cap);
            }
            _ => {
                eprintln!(
                    "microkit: capdl: error: ArchMethods::map_page() received a non-Frame cap: {:?}, for mapping at 0x{:x}, to pd {}",
                    frame_cap, vaddr, pd_name
                );
                std::process::exit(1);
            }
        }

        Ok(())
    }
}
