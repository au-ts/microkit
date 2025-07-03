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
    fn vspace(pd_name: &str) -> NamedObject;

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

fn map_intermediary_level_helper(
    spec: &mut CapDLSpec,
    pd_name: &str,
    next_level_name_prefix: &str,
    cur_level_obj_id: ObjectId,
    cur_level: u8,
    cur_level_slot: u64,
) -> ObjectId {
    let page_table_level_obj_wrapper = spec.get_root_object(cur_level_obj_id).unwrap();
    if let Object::PageTable(page_table_object) = &page_table_level_obj_wrapper.object {
        match page_table_object
            .slots
            .iter()
            .find(|cte| cte.0 == cur_level_slot as usize)
        {
            Some(cte_unwrapped) => {
                // Next level object already created, nothing to do here
                return cte_unwrapped.1.obj();
            }
            None => {
                // We need to create the next level paging structure, get out of this scope for now
                // so we don't get a double mutable borrow of spec when we need to insert the next level object
            }
        }
    } else {
        eprintln!(
            "CapDL spec up to point of error: {}",
            serde_json::to_string_pretty(spec).unwrap()
        );
        eprintln!(
            "microkit: capdl: error: map_intermediary_level_helper() received a non-Page Table cap: {}, for mapping at level {}, to pd {}.",
            cur_level_obj_id, cur_level, pd_name
        );
        std::process::exit(1);
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
    insert_cap_into_page_table_level(spec, cur_level_obj_id, cur_level, cur_level_slot, next_level_cap);

    next_level_obj_id
}

fn insert_cap_into_page_table_level(
    spec: &mut CapDLSpec,
    cur_level_obj_id: ObjectId,
    cur_level: u8,
    cur_level_slot: u64,
    cap: Cap,
) {
    // @billn revisit the error handling, not very transparent
    let page_table_level_obj_wrapper = spec.get_root_object_mut(cur_level_obj_id).unwrap();
    if let Object::PageTable(page_table_object) = &mut page_table_level_obj_wrapper.object {
        // Sanity check that this slot is free
        match page_table_object
            .slots
            .iter()
            .find(|cte| cte.0 == cur_level_slot as usize)
        {
            Some(cte_unwrapped) => {
                page_table_object.slots.push((cur_level_slot as usize, cap));
            }
            None => {
                eprintln!(
                    "CapDL spec up to point of error: {}",
                    serde_json::to_string_pretty(spec).unwrap()
                );
                eprintln!(
                    "microkit: capdl: error: insert_cap_into_page_table_level() slot {} at level {} already filled",
                    cur_level_slot, cur_level
                );
                std::process::exit(1);
            }
        }
    } else {
        eprintln!(
            "CapDL spec up to point of error: {}",
            serde_json::to_string_pretty(spec).unwrap()
        );
        eprintln!(
            "microkit: capdl: error: insert_cap_into_page_table_level() received a non-Page Table cap: {}",
            cur_level_obj_id
        );
        std::process::exit(1);
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
        vspace_obj_id: ObjectId,
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
                    map_intermediary_level_helper(spec, pd_name, "pdpt", vspace_obj_id, 0, pml4_slot);
                let pd_obj_id: ObjectId =
                    map_intermediary_level_helper(spec, pd_name, "pd", pdpt_obj_id, 1, pdpt_slot);
                let pt_obj_id: ObjectId =
                    map_intermediary_level_helper(spec, pd_name, "pt", pd_obj_id, 2, pd_slot);
                insert_cap_into_page_table_level(spec, pt_obj_id, 3, pt_slot, frame_cap);
            }
            _ => {
                eprintln!(
                    "CapDL spec up to point of error: {}",
                    serde_json::to_string_pretty(spec).unwrap()
                );
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
