//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//
use core::ops::Range;
use std::collections::{HashMap, HashSet};

use crate::{capdl::{spec::{FrameInit}, CapDLSpec}, sel4::PageSize, util::round_down};

#[derive(Debug)]
struct Page {
    perms: u8,
    size: PageSize,
    fill: FrameInit
}

#[derive(Debug)]
pub struct PageCollection {
    pages: HashMap<u64, Page>,
}

impl PageCollection {
    pub fn new() -> PageCollection {
        PageCollection {
            pages: HashMap::new(),
        }
    }

    pub fn add_page(&mut self, vaddr: u64, perms: u8, size: PageSize, fill: FrameInit) {
        let round_vaddr = round_down(vaddr, size as u64);
        if !self.pages.contains_key(&round_vaddr) {
            let page = Page {
                perms: perms,
                size: size,
                fill: fill,
            };
            self.pages.insert(round_vaddr, page);
        }
    }

    pub fn to_spec(mut self) -> CapDLSpec {
        let mut spec = CapDLSpec{
            objects: HashSet::new(),
            irqs: HashSet::new(),
            asid_slots: HashSet::new(),
            root_objects: Range { start: 0, end: 0 },
            untyped_covers: HashSet::new(),
        };



        spec
    }
}