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
    // Create an architecture specific VSpace object for insertion into a CapDL spec.
    fn create_vspace(spec: &mut CapDLSpec, pd_name: &str) -> ObjectId;

    // Map the given frame into the given VSpace object ID.
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

fn insert_cap_into_page_table_level(
    spec: &mut CapDLSpec,
    cur_level_obj_id: ObjectId,
    cur_level: u8,
    cur_level_slot: u64,
    cap: Cap,
) -> Result<(), String> {
    let page_table_level_obj_wrapper = spec.get_root_object_mut(cur_level_obj_id).unwrap();
    if let Object::PageTable(page_table_object) = &mut page_table_level_obj_wrapper.object {
        // Sanity check that this slot is free
        match page_table_object
            .slots
            .iter()
            .find(|cte| cte.0 == cur_level_slot as usize)
        {
            Some(_) => Err(format!(
                "insert_cap_into_page_table_level(): slot {} at level {} already filled",
                cur_level_slot, cur_level
            )),
            None => {
                page_table_object.slots.push((cur_level_slot as usize, cap));
                Ok(())
            }
        }
    } else {
        Err(format!(
            "insert_cap_into_page_table_level(): received a non-Page Table cap: {}",
            cur_level_obj_id
        ))
    }
}

fn map_intermediary_level_helper(
    spec: &mut CapDLSpec,
    pd_name: &str,
    next_level_name_prefix: &str,
    cur_level_obj_id: ObjectId,
    cur_level: u8,
    cur_level_slot: u64,
) -> Result<ObjectId, String> {
    let page_table_level_obj_wrapper = spec.get_root_object(cur_level_obj_id).unwrap();
    if let Object::PageTable(page_table_object) = &page_table_level_obj_wrapper.object {
        match page_table_object
            .slots
            .iter()
            .find(|cte| cte.0 == cur_level_slot as usize)
        {
            Some(cte_unwrapped) => {
                // Next level object already created, nothing to do here
                return Ok(cte_unwrapped.1.obj());
            }
            None => {
                // We need to create the next level paging structure, get out of this scope for now
                // so we don't get a double mutable borrow of spec when we need to insert the next level object
            }
        }
    } else {
        return Err(format!("map_intermediary_level_helper() received a non-Page Table cap: {}, for mapping at level {}, to pd {}.",
            cur_level_obj_id, cur_level, pd_name));
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
    let next_level_cap = Cap::PageTable(cap::PageTable {
        object: next_level_obj_id,
    });

    // Then insert into the correct slot at the current level, return and continue mapping
    match insert_cap_into_page_table_level(
        spec,
        cur_level_obj_id,
        cur_level,
        cur_level_slot,
        next_level_cap,
    ) {
        Ok(_) => Ok(next_level_obj_id),
        Err(err_reason) => Err(err_reason),
    }
}

impl ArchMethods for X86_64 {
    fn create_vspace(spec: &mut CapDLSpec, pd_name: &str) -> ObjectId {
        spec.add_root_object(NamedObject {
            name: format!("pml4_{}", pd_name),
            object: Object::PageTable(object::PageTable {
                is_root: true,
                level: Some(0),
                slots: [].to_vec(),
            }),
        })
    }

    fn map_page(
        spec: &mut CapDLSpec,
        pd_name: &str,
        vspace_obj_id: ObjectId,
        frame_cap: Cap,
        frame_size: PageSize,
        vaddr: u64,
    ) -> Result<(), String> {
        match &frame_cap {
            Cap::Frame(_) => {
                assert!(vaddr % frame_size as u64 == 0);

                let frame_obj_id = frame_cap.obj();

                // Get slot indexes for the 4 levels of the page table
                let pml4_slot = (vaddr >> (12 + 9 + 9 + 9)) & ((1 << 9) - 1);
                let pdpt_slot = (vaddr >> (12 + 9 + 9)) & ((1 << 9) - 1);
                let pd_slot = (vaddr >> (12 + 9)) & ((1 << 9) - 1);
                let pt_slot = (vaddr >> (12)) & ((1 << 9) - 1);

                match map_intermediary_level_helper(
                    spec,
                    pd_name,
                    "pdpt",
                    vspace_obj_id,
                    0,
                    pml4_slot,
                ) {
                    Ok(pdpt_obj_id) => {
                        match map_intermediary_level_helper(
                            spec,
                            pd_name,
                            "pd",
                            pdpt_obj_id,
                            1,
                            pdpt_slot,
                        ) {
                            Ok(pd_obj_id) => {
                                match frame_size {
                                    PageSize::Small => {
                                        match map_intermediary_level_helper(
                                            spec, pd_name, "pt", pd_obj_id, 2, pd_slot,
                                        ) {
                                            Ok(pt_obj_id) => {
                                                match insert_cap_into_page_table_level(
                                                    spec, pt_obj_id, 3, pt_slot, frame_cap,
                                                ) {
                                                    Ok(_) => Ok(()),
                                                    Err(lvl3_small_err_reason) => Err(format!("map_page() failed to map small frame {} at vaddr 0x{:x} on page table level 3 to pd {} because: {}", frame_obj_id, vaddr, pd_name, lvl3_small_err_reason)),
                                                }
                                            }
                                            Err(lvl2_err_reason) => Err(format!("map_page() failed to map frame {} at vaddr 0x{:x} on page table level 2 to pd {} because: {}", frame_obj_id, vaddr, pd_name, lvl2_err_reason)),
                                        }
                                    },
                                    PageSize::Large => {
                                        match insert_cap_into_page_table_level(
                                            spec, pd_obj_id, 2, pd_slot, frame_cap,
                                        ) {
                                            Ok(_) => Ok(()),
                                            Err(lvl2_large_err_reason) => Err(format!("map_page() failed to map large frame {} at vaddr 0x{:x} on page table level 2 to pd {} because: {}", frame_obj_id, vaddr, pd_name, lvl2_large_err_reason)),
                                        }
                                    },
                                }
                            }
                            Err(lvl1_err_reason) => Err(format!("map_page() failed to map frame {} at vaddr 0x{:x} on page table level 1 to pd {} because: {}", frame_obj_id, vaddr, pd_name, lvl1_err_reason)),
                        }
                    }
                    Err(lvl0_err_reason) => Err(format!("map_page() failed to map frame {} at vaddr 0x{:x} on page table level 0 to pd {} because: {}", frame_obj_id, vaddr, pd_name, lvl0_err_reason)),
                }
            }
            _ => Err(format!(
                "map_page() received a non-Frame object: {:?}, for mapping at vaddr 0x{:x}, to pd {}",
                frame_cap.obj(), vaddr, pd_name
            )),
        }
    }
}
