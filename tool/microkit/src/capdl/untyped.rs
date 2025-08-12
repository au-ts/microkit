//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use std::{cmp::min};

use crate::{
    capdl::SLOT_SIZE,
    sel4::{Config, Object, ObjectType},
    util::{self, human_size_strict},
    FindFixedError, ObjectAllocator, UntypedObject,
};

pub struct InitSystem<'a> {
    config: &'a Config,
    cnode_cap: u64,
    cap_slot: u64,
    last_fixed_address: u64,
    normal_untyped: ObjectAllocator,
    device_untyped: ObjectAllocator,
    objects: Vec<Object>,
}

impl<'a> InitSystem<'a> {
    pub fn new(
        config: &'a Config,
        cnode_cap: u64,
        first_available_cap_slot: u64,
        normal_untyped: ObjectAllocator,
        device_untyped: ObjectAllocator,
    ) -> InitSystem<'a> {
        InitSystem {
            config,
            cnode_cap,
            cap_slot: first_available_cap_slot,
            last_fixed_address: 0,
            normal_untyped,
            device_untyped,
            objects: Vec::new(),
        }
    }

    pub fn reserve(&mut self, allocations: Vec<(&UntypedObject, u64)>) {
        for alloc in allocations {
            self.device_untyped.reserve(alloc);
        }
    }

    /// Note: Fixed objects must be allocated in order!
    pub fn allocate_fixed_object(
        &mut self,
        phys_address: u64,
        object_type: ObjectType,
        name: String,
    ) -> Object {
        assert!(phys_address >= self.last_fixed_address);
        assert!(object_type.fixed_size(self.config).is_some());

        let alloc_size = object_type.fixed_size(self.config).unwrap();

        // Find an untyped that contains the given address, it could either be
        // in device memory or normal memory.
        let device_ut = self.device_untyped.find_fixed(phys_address, alloc_size).unwrap_or_else(|err| {
            match err {
                FindFixedError::AlreadyAllocated => eprintln!("ERROR: attempted to allocate object '{name}' at 0x{phys_address:x} from reserved region, pick another physical address"),
                FindFixedError::TooLarge => eprintln!("ERROR: attempted too allocate too large of an object '{name}' for this physical address 0x{phys_address:x}"),
            }
            std::process::exit(1);
        });
        let normal_ut = self.normal_untyped.find_fixed(phys_address, alloc_size).unwrap_or_else(|err| {
            match err {
                FindFixedError::AlreadyAllocated => eprintln!("ERROR: attempted to allocate object '{name}' at 0x{phys_address:x} from reserved region, pick another physical address"),
                FindFixedError::TooLarge => eprintln!("ERROR: attempted too allocate too large of an object '{name}' for this physical address 0x{phys_address:x}"),
            }
            std::process::exit(1);
        });

        // We should never have found the physical address in both device and normal untyped
        assert!(!(device_ut.is_some() && normal_ut.is_some()));

        let (padding, ut) = if let Some(x) = device_ut {
            x
        } else if let Some(x) = normal_ut {
            x
        } else {
            eprintln!(
                "ERROR: physical address 0x{phys_address:x} not in any valid region, below are the valid ranges of memory to be allocated from:"
            );
            eprintln!("valid ranges outside of main memory:");
            for ut in &self.device_untyped.untyped {
                eprintln!("     [0x{:0>12x}..0x{:0>12x})", ut.base(), ut.end());
            }
            eprintln!("valid ranges within main memory:");
            for ut in &self.normal_untyped.untyped {
                eprintln!("     [0x{:0>12x}..0x{:0>12x})", ut.base(), ut.end());
            }
            std::process::exit(1);
        };

        if let Some(padding_unwrapped) = padding {
            for pad_ut in padding_unwrapped {
                // self.invocations.push(Invocation::new(
                //     self.config,
                //     InvocationArgs::UntypedRetype {
                //         untyped: pad_ut.untyped_cap_address,
                //         object_type: ObjectType::Untyped,
                //         size_bits: pad_ut.size.ilog2() as u64,
                //         root: self.cnode_cap,
                //         node_index: 1,
                //         node_depth: 1,
                //         node_offset: self.cap_slot,
                //         num_objects: 1,
                //     },
                // ));
                self.cap_slot += 1;
            }
        }

        let object_cap = self.cap_slot;
        self.cap_slot += 1;
        // self.invocations.push(Invocation::new(
        //     self.config,
        //     InvocationArgs::UntypedRetype {
        //         untyped: ut.untyped_cap_address,
        //         object_type,
        //         size_bits: 0,
        //         root: self.cnode_cap,
        //         node_index: 1,
        //         node_depth: 1,
        //         node_offset: object_cap,
        //         num_objects: 1,
        //     },
        // ));

        self.last_fixed_address = phys_address + alloc_size;
        // let cap_addr = self.cnode_mask | object_cap;
        let cap_addr = object_cap;
        let kernel_object = Object {
            object_type,
            cap_addr,
            phys_addr: phys_address,
        };
        self.objects.push(kernel_object);
        // self.cap_address_names.insert(cap_addr, name);

        kernel_object
    }

    pub fn allocate_objects(
        &mut self,
        object_type: ObjectType,
        names: Vec<String>,
        size: Option<u64>,
    ) -> Vec<Object> {
        // Nothing to do if we get a zero count.
        if names.is_empty() {
            return Vec::new();
        }

        let count = names.len() as u64;

        let alloc_size;
        let api_size: u64;
        if let Some(object_size) = object_type.fixed_size(self.config) {
            // An object with a fixed size should not be allocated with a given size
            assert!(size.is_none());
            alloc_size = object_size;
            api_size = 0;
        } else if object_type == ObjectType::CNode || object_type == ObjectType::SchedContext {
            let sz = size.unwrap();
            assert!(util::is_power_of_two(sz));
            api_size = sz.ilog2() as u64;
            if object_type == ObjectType::CNode {
                alloc_size = sz * SLOT_SIZE;
            } else {
                alloc_size = sz;
            }
        } else {
            panic!("Internal error: invalid object type: {object_type:?}");
        }

        let allocation = self.normal_untyped
                             .alloc_n(alloc_size, count)
                             .unwrap_or_else(|| {
                                    let human_size = human_size_strict(alloc_size * count);
                                    let human_max_alloc = human_size_strict(self.normal_untyped.max_alloc_size());
                                    eprintln!("ERROR: failed to allocate objects for '{}' of object type '{}'", names[0], object_type.to_str());
                                    if alloc_size * count > self.normal_untyped.max_alloc_size() {
                                        eprintln!("ERROR: allocation size ({human_size}) is greater than current maximum size for a single allocation ({human_max_alloc})");
                                    }
                                    std::process::exit(1);
                                }
                             );
        let base_cap_slot = self.cap_slot;
        self.cap_slot += count;

        let mut to_alloc = count;
        let mut alloc_cap_slot = base_cap_slot;
        while to_alloc > 0 {
            let call_count = min(to_alloc, self.config.fan_out_limit);
            // self.invocations.push(Invocation::new(
            //     self.config,
            //     InvocationArgs::UntypedRetype {
            //         untyped: allocation.untyped_cap_address,
            //         object_type,
            //         size_bits: api_size,
            //         root: self.cnode_cap,
            //         node_index: 1,
            //         node_depth: 1,
            //         node_offset: alloc_cap_slot,
            //         num_objects: call_count,
            //     },
            // ));
            to_alloc -= call_count;
            alloc_cap_slot += call_count;
        }

        let mut kernel_objects = Vec::new();
        let mut phys_addr = allocation.phys_addr;
        for (idx, name) in names.into_iter().enumerate() {
            let cap_slot = base_cap_slot + idx as u64;
            // let cap_addr = self.cnode_mask | cap_slot;
            let cap_addr = cap_slot;
            let kernel_object = Object {
                object_type,
                cap_addr,
                phys_addr,
            };
            kernel_objects.push(kernel_object);
            // self.cap_address_names.insert(cap_addr, name);

            phys_addr += alloc_size;

            self.objects.push(kernel_object);
        }

        kernel_objects
    }
}
