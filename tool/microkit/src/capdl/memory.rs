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
    sel4::{Arch, Config, PageSize},
};

/// For naming and debugging purposes only, no functional purpose.
fn get_pt_level_name(sel4_config: &Config, level: u64) -> &str {
    match sel4_config.arch {
        crate::sel4::Arch::Aarch64 => match level {
            0 => "pgd",
            1 => "pud",
            2 => "pd",
            3 => "pt",
            _ => unreachable!("unknown page table level {} for aarch64", level),
        },
        crate::sel4::Arch::Riscv64 => match level {
            0 => "pgd",
            1 => "pmd",
            2 => "pte",
            _ => unreachable!("unknown page table level {} for riscv64", level),
        },
        crate::sel4::Arch::X86_64 => match level {
            0 => "pml4",
            1 => "pdpt",
            2 => "pd",
            3 => "pt",
            _ => unreachable!("unknown page table level {} for x86_64", level),
        },
    }
}

fn get_pt_level_index(sel4_config: &Config, level: usize, vaddr: u64) -> u64 {
    assert!(level < sel4_config.num_page_table_levels());

    if level == 0 && sel4_config.arch == Arch::Aarch64 && sel4_config.aarch64_vspace_s2_start_l1() {
        // Special case for first level on AArch64 platforms with hyp and 40 bits PA.
        // It have 10 bits index for VSpace.
        // match up with seL4_VSpaceBits in seL4/libsel4/sel4_arch_include/aarch64/sel4/sel4_arch/constants.h
        return (vaddr >> (12 + 9 + 9 + 9)) & ((1 << 10) - 1);
    }

    match sel4_config.num_page_table_levels() {
        3 => {
            match level {
                0 => (vaddr >> (12 + 9 + 9)) & ((1 << 9) - 1),
                1 => (vaddr >> (12 + 9)) & ((1 << 9) - 1),
                2 => (vaddr >> (12)) & ((1 << 9) - 1),
                _ => unreachable!("should never reach here as we've validated the level number.")
            }
        },
        4 => {
            match level {
                0 => (vaddr >> (12 + 9 + 9 + 9)) & ((1 << 9) - 1),
                1 => (vaddr >> (12 + 9 + 9)) & ((1 << 9) - 1),
                2 => (vaddr >> (12 + 9)) & ((1 << 9) - 1),
                3 => (vaddr >> (12)) & ((1 << 9) - 1),
                _ => unreachable!("should never reach here as we've validated the level number.")
            }
        },
        _ => unreachable!("should never reach here as we assume either a 3 or 4 levels page table.")
    }
}

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
        name: format!(
            "{}_{}_slot_{:03}_from_obj_id_{}",
            next_level_name_prefix, pd_name, cur_level_slot, cur_level_obj_id
        ),
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

pub fn create_vspace(spec: &mut CapDLSpec, sel4_config: &Config, pd_name: &str) -> ObjectId {
    spec.add_root_object(NamedObject {
        name: format!("{}_{}", get_pt_level_name(sel4_config, 0), pd_name),
        object: Object::PageTable(object::PageTable {
            is_root: true,
            level: Some(0),
            slots: [].to_vec(),
        }),
    })
}

fn map_4_levels(
    spec: &mut CapDLSpec,
    sel4_config: &Config,
    pd_name: &str,
    vspace_obj_id: ObjectId,
    frame_cap: Cap,
    frame_size: PageSize,
    vaddr: u64,
) -> Result<(), String> {
    match &frame_cap {
        Cap::Frame(_) => {
            assert_eq!(vaddr % frame_size as u64, 0);

            let frame_obj_id = frame_cap.obj();

            // Get slot indexes for the 4 levels of the page table
            let lv0_slot = get_pt_level_index(sel4_config, 0, vaddr);
            let lv1_slot = get_pt_level_index(sel4_config, 1, vaddr);
            let lv2_slot = get_pt_level_index(sel4_config, 2, vaddr);
            let lv3_slot = get_pt_level_index(sel4_config, 3, vaddr);

            match map_intermediary_level_helper(
                spec,
                pd_name,
                get_pt_level_name(sel4_config, 1),
                vspace_obj_id,
                0,
                lv0_slot,
            ) {
                Ok(lvl1_obj_id) => {
                    match map_intermediary_level_helper(
                        spec,
                        pd_name,
                        get_pt_level_name(sel4_config, 2),
                        lvl1_obj_id,
                        1,
                        lv1_slot,
                    ) {
                        Ok(lvl2_obj_id) => {
                            match frame_size {
                                PageSize::Small => {
                                    match map_intermediary_level_helper(
                                        spec, pd_name, get_pt_level_name(sel4_config, 3), lvl2_obj_id, 2, lv2_slot,
                                    ) {
                                        Ok(lvl3_obj_id) => {
                                            match insert_cap_into_page_table_level(
                                                spec, lvl3_obj_id, 3, lv3_slot, frame_cap,
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
                                        spec, lvl2_obj_id, 2, lv2_slot, frame_cap,
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
        _ => {
            Err(format!(
            "map_page() received a non-Frame object: {:?}, for mapping at vaddr 0x{:x}, to pd {}",
            frame_cap.obj(), vaddr, pd_name
        ))
        }
    }
}

fn map_3_levels(
    spec: &mut CapDLSpec,
    sel4_config: &Config,
    pd_name: &str,
    vspace_obj_id: ObjectId,
    frame_cap: Cap,
    frame_size: PageSize,
    vaddr: u64,
) -> Result<(), String> {
    match &frame_cap {
        Cap::Frame(_) => {
            assert_eq!(vaddr % frame_size as u64, 0);

            let frame_obj_id = frame_cap.obj();

            // Get slot indexes for the 3 levels of the page table
            let lv0_slot = get_pt_level_index(sel4_config, 0, vaddr);
            let lv1_slot = get_pt_level_index(sel4_config, 1, vaddr);
            let lv2_slot = get_pt_level_index(sel4_config, 2, vaddr);

            match map_intermediary_level_helper(
                spec,
                pd_name,
                get_pt_level_name(sel4_config, 1),
                vspace_obj_id,
                0,
                lv0_slot,
            ) {
                Ok(lvl1_obj_id) => {
                    match frame_size {
                        PageSize::Small => {
                            match map_intermediary_level_helper(
                                spec, pd_name, get_pt_level_name(sel4_config, 2), lvl1_obj_id, 1, lv1_slot,
                            ) {
                                Ok(lvl2_obj_id) => {
                                    match insert_cap_into_page_table_level(
                                        spec, lvl2_obj_id, 2, lv2_slot, frame_cap,
                                    ) {
                                        Ok(_) => Ok(()),
                                        Err(lvl2_small_err_reason) => Err(format!("map_page() failed to map small frame {} at vaddr 0x{:x} on page table level 2 to pd {} because: {}", frame_obj_id, vaddr, pd_name, lvl2_small_err_reason)),
                                    }
                                }
                                Err(lvl2_err_reason) => Err(format!("map_page() failed to map frame {} at vaddr 0x{:x} on page table level 1 to pd {} because: {}", frame_obj_id, vaddr, pd_name, lvl2_err_reason)),
                            }
                        },
                        PageSize::Large => {
                            match insert_cap_into_page_table_level(
                                spec, lvl1_obj_id, 1, lv1_slot, frame_cap,
                            ) {
                                Ok(_) => Ok(()),
                                Err(lvl1_large_err_reason) => Err(format!("map_page() failed to map large frame {} at vaddr 0x{:x} on page table level 1 to pd {} because: {}", frame_obj_id, vaddr, pd_name, lvl1_large_err_reason)),
                            }
                        },
                    }
                }
                Err(lvl0_err_reason) => Err(format!("map_page() failed to map frame {} at vaddr 0x{:x} on page table level 0 to pd {} because: {}", frame_obj_id, vaddr, pd_name, lvl0_err_reason)),
            }
        }
        _ => {
            Err(format!(
            "map_page() received a non-Frame object: {:?}, for mapping at vaddr 0x{:x}, to pd {}",
            frame_cap.obj(), vaddr, pd_name
        ))
        }
    }
}

pub fn map_page(
    spec: &mut CapDLSpec,
    sel4_config: &Config,
    pd_name: &str,
    vspace_obj_id: ObjectId,
    frame_cap: Cap,
    frame_size: PageSize,
    vaddr: u64,
) -> Result<(), String> {
    if sel4_config.arch == Arch::Riscv64 {
        map_3_levels(
            spec,
            sel4_config,
            pd_name,
            vspace_obj_id,
            frame_cap,
            frame_size,
            vaddr,
        )
    } else {
        map_4_levels(
            spec,
            sel4_config,
            pd_name,
            vspace_obj_id,
            frame_cap,
            frame_size,
            vaddr,
        )
    }
}
