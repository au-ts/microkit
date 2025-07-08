//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use crate::capdl::{
    spec::{cap, object, Cap, FrameInit, NamedObject, Object, ObjectId, Rights},
    CapDLSpec,
};

/// This module contains utility functions used by higher-level
/// CapDL spec generation code.

/// Create a frame object and add it to the spec, returns the
/// object number.
pub fn capdl_util_make_frame_obj(
    spec: &mut CapDLSpec,
    frame_init: FrameInit,
    name: &str,
    paddr: Option<usize>
) -> ObjectId {
    let frame_inner_obj = Object::Frame(object::Frame {
        size_bits: 12, // @billn fix use ObjectType::fixed_size_bits
        paddr,
        init: frame_init,
    });
    let frame_obj = NamedObject {
        name: format!("frame_{}", name),
        object: frame_inner_obj,
    };
    spec.add_root_object(frame_obj)
}

/// Create a frame capability from a frame object for mapping in the frame
pub fn capdl_util_make_frame_cap(
    frame_obj_id: ObjectId,
    read: bool,
    write: bool,
    execute: bool,
    cached: bool,
) -> Cap {
    Cap::Frame(cap::Frame {
        object: frame_obj_id,
        rights: Rights {
            read,
            write,
            grant: execute,
            grant_reply: false, // @billn what is this used for??
        },
        cached,
    })
}

pub fn capdl_util_get_vspace_id_from_tcb_id(spec: &CapDLSpec, tcb_obj_id: ObjectId) -> ObjectId {
    let tcb = match spec.get_root_object(tcb_obj_id) {
        Some(named_object) => {
            if let Object::Tcb(tcb) = &named_object.object {
                Some(tcb)
            } else {
                unreachable!("internal bug: get_vspace_id_from_tcb_id() got a non TCB object ID.");
            }
        }
        None => {
            unreachable!();
        }
    };
    let vspace_cap = tcb.unwrap().slots.iter().find(|&cte| {
        if let Cap::PageTable(_) = &cte.1 {
            true
        } else {
            false
        }
    });
    vspace_cap.unwrap().1.obj()
}
