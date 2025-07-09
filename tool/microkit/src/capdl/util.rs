//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use crate::capdl::{
    spec::{
        cap,
        object::{self, SchedContextExtraInfo},
        Cap, CapTableEntry, FrameInit, NamedObject, Object, ObjectId, Rights,
    },
    CapDLSpec,
};

/// This module contains utility functions used by higher-level
/// CapDL spec generation code. For simplicity, this code will trust
/// all arguments given to it as it is only meant to be used internally
/// in the CapDL implementation.

/// Create a frame object and add it to the spec, returns the
/// object number.
pub fn capdl_util_make_frame_obj(
    spec: &mut CapDLSpec,
    frame_init: FrameInit,
    name: &str,
    paddr: Option<usize>,
    size_bits: usize,
) -> ObjectId {
    let frame_inner_obj = Object::Frame(object::Frame {
        size_bits: size_bits,
        paddr,
        init: frame_init,
    });
    let frame_obj = NamedObject {
        name: format!("frame_{}", name),
        object: frame_inner_obj,
    };
    spec.add_root_object(frame_obj)
}

/// Create a frame capability from a frame object for mapping the frame in a VSpace
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
            grant_reply: false,
        },
        cached,
    })
}

// Given a TCB object ID, return that TCB's VSpace object ID.
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
            unreachable!(
                "internal bug: get_vspace_id_from_tcb_id() couldn't find tcb with given obj id."
            );
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

pub fn capdl_util_make_endpoint_obj(spec: &mut CapDLSpec, pd_name: &str) -> ObjectId {
    let fault_ep_obj = NamedObject {
        name: format!("ep_fault_{}", pd_name).to_string(),
        object: Object::Endpoint,
    };
    spec.add_root_object(fault_ep_obj)
}

pub fn capdl_util_make_endpoint_cap(
    ep_obj_id: ObjectId,
    read: bool,
    write: bool,
    grant: bool,
    badge: u64,
) -> Cap {
    Cap::Endpoint(cap::Endpoint {
        object: ep_obj_id,
        badge,
        rights: Rights {
            read,
            write,
            grant,
            grant_reply: false,
        },
    })
}

pub fn capdl_util_make_reply_obj(spec: &mut CapDLSpec, pd_name: &str) -> ObjectId {
    let reply_obj = NamedObject {
        name: format!("reply_{}", pd_name).to_string(),
        object: Object::Reply,
    };
    spec.add_root_object(reply_obj)
}

pub fn capdl_util_make_reply_cap(reply_obj_id: ObjectId) -> Cap {
    Cap::Reply(cap::Reply {
        object: reply_obj_id,
    })
}

pub fn capdl_util_make_sc_obj(
    spec: &mut CapDLSpec,
    pd_name: &str,
    size_bits: usize,
    period: u64,
    budget: u64,
    badge: u64,
) -> ObjectId {
    let sc_inner_obj = Object::SchedContext(object::SchedContext {
        size_bits,
        extra: SchedContextExtraInfo {
            period,
            budget,
            badge,
        },
    });
    let sc_obj = NamedObject {
        name: format!("sched_context_{}", pd_name).to_string(),
        object: sc_inner_obj,
    };
    spec.add_root_object(sc_obj)
}

pub fn capdl_util_make_sc_cap(sc_obj_id: ObjectId) -> Cap {
    Cap::SchedContext(cap::SchedContext { object: sc_obj_id })
}

pub fn capdl_util_make_cnode_obj(
    spec: &mut CapDLSpec,
    pd_name: &str,
    size_bits: usize,
    slots: Vec<CapTableEntry>,
) -> ObjectId {
    let cnode_inner_obj = Object::CNode(object::CNode { size_bits, slots });
    let cnode_obj = NamedObject {
        name: format!("cnode_{}", pd_name).to_string(),
        object: cnode_inner_obj,
    };
    // Move monitor CSpace into spec and make a cap for it to insert into TCB later.
    spec.add_root_object(cnode_obj)
}

pub fn capdl_util_make_cnode_cap(cnode_obj_id: ObjectId, guard: u64, guard_size: u64) -> Cap {
    Cap::CNode(cap::CNode {
        object: cnode_obj_id,
        guard,

        guard_size,
    })
}
