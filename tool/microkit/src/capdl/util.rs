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
) -> ObjectId {
    let frame_inner_obj = Object::Frame(object::Frame {
        size_bits: 12, // @billn fix use ObjectType::fixed_size_bits
        paddr: None,
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
