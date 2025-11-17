//
// Copyright 2024, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use std::{cmp::min, fmt};

use crate::{
    sel4::{Config, PageSize},
    util::struct_to_bytes,
};

use zerocopy::{IntoBytes,Immutable};

pub mod capdl;
pub mod crc32;
pub mod elf;
pub mod loader;
pub mod report;
pub mod sdf;
pub mod sel4;
pub mod symbols;
pub mod uimage;
pub mod util;

// Note that these values are used in the monitor so should also be changed there
// if any of these were to change.
pub const MAX_PDS: usize = 63;
pub const MAX_VMS: usize = 63;
// It should be noted that if you were to change the value of
// the maximum PD/VM name length, you would also have to change
// the monitor and libmicrokit.
pub const PD_MAX_NAME_LENGTH: usize = 64;
pub const VM_MAX_NAME_LENGTH: usize = 64;

// Note that these constants align with the only architectures that we are
// supporting at the moment
pub const PAGE_TABLE_ENTRIES: u64 = 512;
pub const PAGE_TABLE_MASK: u64 = 0x1ff;
pub enum PageTableMaskShift {
    PGD = 39,
    PUD = 30,
    PD = 21,
    PT = 12,
}

#[derive(IntoBytes, Immutable)]
#[repr(C)]
pub struct TableMetadata {
    pub base_addr: u64,
    pub pgd: [u64; 64],
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PGD {
    puds: Vec<Option<PUD>>,
}

impl Default for PGD {
    fn default() -> Self {
        Self::new()
    }
}

impl PGD {
    pub fn new() -> Self {
        PGD {
            puds: vec![None; PAGE_TABLE_ENTRIES as usize],
        }
    }

    pub fn recurse(&mut self, mut curr_offset: u64, buffer: &mut Vec<u8>) -> u64 {
        let mut offset_table: [u64; PAGE_TABLE_ENTRIES as usize] = [u64::MAX; PAGE_TABLE_ENTRIES as usize];
        for (i, entry) in offset_table.iter_mut().enumerate() {
            if let Some(pud) = &mut self.puds[i] {
                curr_offset = pud.recurse(curr_offset, buffer);
                *entry = curr_offset - (PAGE_TABLE_ENTRIES * 8);
            }
        }

        for value in &mut offset_table {
            buffer.append(&mut value.to_le_bytes().to_vec());
        }
        curr_offset + (PAGE_TABLE_ENTRIES * 8)
    }

    pub fn add_page_at_vaddr(&mut self, vaddr: u64, frame: u64, size: u64) {
        let pgd_index = ((vaddr & (PAGE_TABLE_MASK << PageTableMaskShift::PGD as u64)) >> PageTableMaskShift::PGD as u64) as usize;
        if self.puds[pgd_index].is_none() {
            self.puds[pgd_index] = Some(PUD::new());
        }
        self.puds[pgd_index]
            .as_mut()
            .unwrap()
            .add_page_at_vaddr(vaddr, frame, size);
    }

    pub fn add_page_at_vaddr_range(
        &mut self,
        mut vaddr: u64,
        mut data_len: i64,
        frame: u64,
        size: u64,
    ) {
        while data_len > 0 {
            self.add_page_at_vaddr(vaddr, frame, size);
            data_len -= size as i64;
            vaddr += size;
        }
    }

    pub fn get_size(&self) -> u64 {
        let mut child_size = 0;
        for pud in &self.puds {
            if pud.is_some() {
                child_size += pud.as_ref().unwrap().get_size();
            }
        }
        (PAGE_TABLE_ENTRIES * 8) + child_size
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PUD {
    dirs: Vec<Option<DIR>>,
}

impl PUD {
    pub fn new() -> Self {
        PUD {
            dirs: vec![None; PAGE_TABLE_ENTRIES as usize],
        }
    }

    pub fn recurse(&mut self, mut curr_offset: u64, buffer: &mut Vec<u8>) -> u64 {
        let mut offset_table: [u64; PAGE_TABLE_ENTRIES as usize] = [u64::MAX; PAGE_TABLE_ENTRIES as usize];
        for (i, entry) in offset_table.iter_mut().enumerate() {
            if let Some(dir) = &mut self.dirs[i] {
                curr_offset = dir.recurse(curr_offset, buffer);
                *entry = curr_offset - (PAGE_TABLE_ENTRIES * 8);
            }
        }

        for value in &mut offset_table {
            buffer.append(&mut value.to_le_bytes().to_vec());
        }
        curr_offset + (PAGE_TABLE_ENTRIES * 8)
    }

    pub fn add_page_at_vaddr(&mut self, vaddr: u64, frame: u64, size: u64) {
        let pud_index = ((vaddr & (PAGE_TABLE_MASK << PageTableMaskShift::PUD as u64)) >> PageTableMaskShift::PUD as u64) as usize;
        if self.dirs[pud_index].is_none() {
            self.dirs[pud_index] = Some(DIR::new());
        }
        self.dirs[pud_index]
            .as_mut()
            .unwrap()
            .add_page_at_vaddr(vaddr, frame, size);
    }

    pub fn add_page_at_vaddr_range(
        &mut self,
        mut vaddr: u64,
        mut data_len: i64,
        frame: u64,
        size: u64,
    ) {
        while data_len > 0 {
            self.add_page_at_vaddr(vaddr, frame, size);
            data_len -= size as i64;
            vaddr += size;
        }
    }

    pub fn get_size(&self) -> u64 {
        let mut child_size = 0;
        for dir in &self.dirs {
            if dir.is_some() {
                child_size += dir.as_ref().unwrap().get_size();
            }
        }
        (PAGE_TABLE_ENTRIES * 8) + child_size
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum DirEntry {
    PageTable(PT),
    LargePage(u64),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DIR {
    entries: Vec<Option<DirEntry>>,
}

impl DIR {
    fn new() -> Self {
        DIR {
            entries: vec![None; PAGE_TABLE_ENTRIES as usize],
        }
    }

    fn recurse(&mut self, mut curr_offset: u64, buffer: &mut Vec<u8>) -> u64 {
        let mut offset_table: [u64; PAGE_TABLE_ENTRIES as usize] = [u64::MAX; PAGE_TABLE_ENTRIES as usize];
        for (i, dir_entry) in offset_table.iter_mut().enumerate() {
            if let Some(entry) = &mut self.entries[i] {
                match entry {
                    DirEntry::PageTable(x) => {
                        curr_offset = x.recurse(curr_offset, buffer);
                        *dir_entry = curr_offset - (PAGE_TABLE_ENTRIES * 8);
                    }
                    DirEntry::LargePage(x) => {
                        // we mark the top bit to signal to the pd that this is a large page
                        *dir_entry = *x | (1 << 63);
                    }
                }
            }
        }

        for value in &mut offset_table {
            buffer.append(&mut value.to_le_bytes().to_vec());
        }
        curr_offset + (PAGE_TABLE_ENTRIES * 8)
    }

    fn add_page_at_vaddr(&mut self, vaddr: u64, frame: u64, size: u64) {
        let dir_index = ((vaddr & (PAGE_TABLE_MASK << PageTableMaskShift::PD as u64)) >> PageTableMaskShift::PD as u64) as usize;
        if size == PageSize::Small as u64 {
            if self.entries[dir_index].is_none() {
                self.entries[dir_index] = Some(DirEntry::PageTable(PT::new()));
            }
            match &mut self.entries[dir_index] {
                Some(DirEntry::PageTable(x)) => {
                    x.add_page_at_vaddr(vaddr, frame, size);
                }
                _ => {
                    panic!("Trying to add small page where a large page already exists!");
                }
            }
        }
        else if size == PageSize::Large as u64 {
            if let Some(DirEntry::PageTable(_)) = self.entries[dir_index] {
                panic!("Attempting to insert a large page where a page table already exists!");
            }
            self.entries[dir_index] = Some(DirEntry::LargePage(frame));
        }
    }

    fn get_size(&self) -> u64 {
        let mut child_size = 0;
        for pt in &self.entries {
            if let Some(DirEntry::PageTable(x)) = pt {
                child_size += x.get_size();
            }
        }
        (PAGE_TABLE_ENTRIES * 8) + child_size
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PT {
    large_page: u64,
    pages: Vec<u64>,
}

impl PT {
    fn new() -> Self {
        PT {
            pages: vec![u64::MAX; PAGE_TABLE_ENTRIES as usize],
            large_page: u64::MAX,
        }
    }

    fn recurse(&mut self, curr_offset: u64, buffer: &mut Vec<u8>) -> u64 {
        for value in &mut self.pages {
            buffer.append(&mut value.to_le_bytes().to_vec());
        }
        curr_offset + (PAGE_TABLE_ENTRIES * 8)
    }

    fn add_page_at_vaddr(&mut self, vaddr: u64, frame: u64, size: u64) {
        let pt_index = ((vaddr & (PAGE_TABLE_MASK << PageTableMaskShift::PT as u64)) >> PageTableMaskShift::PT as u64) as usize;
        // Unconditionally overwrite.
        assert!(size == PageSize::Small as u64);
        self.pages[pt_index] = frame;
    }

    fn get_size(&self) -> u64 {
        PAGE_TABLE_ENTRIES * 8
    }
}

#[derive(Debug, Clone)]
pub enum TopLevelPageTable {
    Riscv64 { top_level: PUD },
    Aarch64 { top_level: PGD},
}

#[derive(Debug, Clone, PartialEq)]
pub struct UntypedObject {
    pub cap: u64,
    pub region: MemoryRegion,
    pub is_device: bool,
}

pub const UNTYPED_DESC_PADDING: usize = size_of::<u64>() - (2 * size_of::<u8>());
#[repr(C)]
struct SeL4UntypedDesc {
    paddr: u64,
    size_bits: u8,
    is_device: u8,
    padding: [u8; UNTYPED_DESC_PADDING],
}

impl From<&UntypedObject> for SeL4UntypedDesc {
    fn from(value: &UntypedObject) -> Self {
        Self {
            paddr: value.base(),
            size_bits: value.size_bits() as u8,
            is_device: if value.is_device { 1 } else { 0 },
            padding: [0u8; UNTYPED_DESC_PADDING],
        }
    }
}

/// Getting a `seL4_UntypedDesc` for patching into the initialiser
pub fn serialise_ut(ut: &UntypedObject) -> Vec<u8> {
    let sel4_untyped_desc: SeL4UntypedDesc = ut.into();
    unsafe { struct_to_bytes(&sel4_untyped_desc).to_vec() }
}

impl UntypedObject {
    pub fn new(cap: u64, region: MemoryRegion, is_device: bool) -> UntypedObject {
        UntypedObject {
            cap,
            region,
            is_device,
        }
    }

    pub fn base(&self) -> u64 {
        self.region.base
    }

    pub fn end(&self) -> u64 {
        self.region.end
    }

    pub fn size_bits(&self) -> u64 {
        util::lsb(self.region.size())
    }
}

pub struct Region {
    pub name: String,
    pub addr: u64,
    pub size: u64,
    // In order to avoid some expensive copies to put the data
    // into this struct, we instead store the index of the segment
    // of the ELF this region is associated with.
    segment_idx: usize,
}

impl Region {
    pub fn new(name: String, addr: u64, size: u64, segment_idx: usize) -> Region {
        Region {
            name,
            addr,
            size,
            segment_idx,
        }
    }

    pub fn data<'a>(&self, elf: &'a elf::ElfFile) -> &'a Vec<u8> {
        elf.segments[self.segment_idx].data()
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "<Region name={} addr=0x{:x} size={}>",
            self.name, self.addr, self.size
        )
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MemoryRegion {
    /// Note: base is inclusive, end is exclusive
    /// MemoryRegion(1, 5) would have a size of 4
    /// and cover [1, 2, 3, 4]
    pub base: u64,
    pub end: u64,
}

impl fmt::Display for MemoryRegion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MemoryRegion(base=0x{:x}, end=0x{:x})",
            self.base, self.end
        )
    }
}

impl MemoryRegion {
    pub fn new(base: u64, end: u64) -> MemoryRegion {
        MemoryRegion { base, end }
    }

    pub fn size(&self) -> u64 {
        self.end - self.base
    }

    pub fn aligned_power_of_two_regions(
        &self,
        config: &Config,
        max_bits: u64,
    ) -> Vec<MemoryRegion> {
        // During the boot phase, the kernel creates all of the untyped regions
        // based on the kernel virtual addresses, rather than the physical
        // memory addresses. This has a subtle side affect in the process of
        // creating untypeds as even though all the kernel virtual addresses are
        // a constant offset of the corresponding physical address, overflow can
        // occur when dealing with virtual addresses. This precisely occurs in
        // this function, causing different regions depending on whether
        // you use kernel virtual or physical addresses. In order to properly
        // emulate the kernel booting process, we also have to emulate the unsigned integer
        // overflow that can occur.
        let mut regions = Vec::new();
        let mut base = config.paddr_to_kernel_vaddr(self.base);
        let end = config.paddr_to_kernel_vaddr(self.end);
        let mut bits;
        while base != end {
            let size = end.wrapping_sub(base);
            let size_bits = util::msb(size);
            if base == 0 {
                bits = size_bits;
            } else {
                bits = min(size_bits, util::lsb(base));
            }

            if bits > max_bits {
                bits = max_bits;
            }
            let sz = 1 << bits;
            let base_paddr = config.kernel_vaddr_to_paddr(base);
            let end_paddr = config.kernel_vaddr_to_paddr(base.wrapping_add(sz));
            regions.push(MemoryRegion::new(base_paddr, end_paddr));
            base = base.wrapping_add(sz);
        }

        regions
    }
}

#[derive(Default, Debug, Clone)]
pub struct DisjointMemoryRegion {
    pub regions: Vec<MemoryRegion>,
}

impl DisjointMemoryRegion {
    fn check(&self) {
        // Ensure that regions are sorted and non-overlapping
        let mut last_end: Option<u64> = None;
        for region in &self.regions {
            if last_end.is_some() {
                assert!(region.base >= last_end.unwrap());
            }
            last_end = Some(region.end)
        }
    }

    pub fn insert_region(&mut self, base: u64, end: u64) {
        assert!(base < end);

        if self.regions.is_empty() {
            self.regions.push(MemoryRegion::new(base, end));
            return;
        }

        let mut insert_idx = self.regions.len();
        for (idx, region) in self.regions.iter().enumerate() {
            if end <= region.base {
                insert_idx = idx;
                break;
            }
        }
        // Merge if contiguous
        if insert_idx == 0 && self.regions.first().unwrap().base == end {
            self.regions.first_mut().unwrap().base = base;
        } else if insert_idx == self.regions.len() && self.regions.last().unwrap().end == base {
            self.regions.last_mut().unwrap().end = end;
        } else if insert_idx < self.regions.len() && end == self.regions[insert_idx].base {
            self.regions[insert_idx].base = base;
        } else if insert_idx < self.regions.len() && base == self.regions[insert_idx].end {
            self.regions[insert_idx].end = end;
        } else {
            self.regions
                .insert(insert_idx, MemoryRegion::new(base, end));
        }
        self.check();
    }

    pub fn remove_region(&mut self, base: u64, end: u64) {
        let mut maybe_idx = None;
        for (i, r) in self.regions.iter().enumerate() {
            if base >= r.base && end <= r.end {
                maybe_idx = Some(i);
                break;
            }
        }
        if maybe_idx.is_none() {
            panic!("Internal error: attempting to remove region [0x{base:x}-0x{end:x}) that is not currently covered");
        }

        let idx = maybe_idx.unwrap();

        let region = self.regions[idx];

        if region.base == base && region.end == end {
            // Covers exactly, so just remove
            self.regions.remove(idx);
        } else if region.base == base {
            // Trim the start of the region
            self.regions[idx] = MemoryRegion::new(end, region.end);
        } else if region.end == end {
            // Trim end of the region
            self.regions[idx] = MemoryRegion::new(region.base, base);
        } else {
            // Splitting
            self.regions[idx] = MemoryRegion::new(region.base, base);
            self.regions
                .insert(idx + 1, MemoryRegion::new(end, region.end));
        }

        self.check();
    }

    pub fn aligned_power_of_two_regions(
        &self,
        config: &Config,
        max_bits: u64,
    ) -> Vec<MemoryRegion> {
        let mut aligned_regions = Vec::new();
        for region in &self.regions {
            aligned_regions.extend(region.aligned_power_of_two_regions(config, max_bits));
        }

        aligned_regions
    }

    /// Allocate region of 'size' bytes, returning the base address.
    /// The allocated region is removed from the disjoint memory region.
    /// Allocation policy is simple first fit in bottom up direction.
    /// Possibly a 'best fit' policy would be better.
    /// 'best' may be something that best matches a power-of-two
    /// allocation
    pub fn allocate(&mut self, size: u64, align_page_sz: PageSize) -> Option<u64> {
        let mut region_to_remove: Option<MemoryRegion> = None;

        for region in self.regions.iter() {
            if size <= region.size()
                && region.base.next_multiple_of(align_page_sz as u64) + size <= region.end
            {
                region_to_remove = Some(*region);
                break;
            }
        }

        // Got a region that fits, block out the target area, split up the remaining region if necessary.
        match region_to_remove {
            Some(region) => {
                let base = region.base.next_multiple_of(align_page_sz as u64);
                self.remove_region(base, base + size);
                Some(base)
            }
            None => None,
        }
    }

    pub fn allocate_from(&mut self, size: u64, lower_bound: u64) -> Option<u64> {
        let mut region_to_remove = None;
        for region in &self.regions {
            if size <= region.size() && region.base >= lower_bound {
                region_to_remove = Some(*region);
                break;
            }
        }

        match region_to_remove {
            Some(region) => {
                self.remove_region(region.base, region.base + size);
                Some(region.base)
            }
            None => None,
        }
    }
}
