use super::consts::*;
use super::pd_vm::ProtectionDomain;
use super::util::{check_attributes, checked_lookup, loc_string, value_error, sdf_parse_number};
use super::{SdfNode, XmlSystemDescription};
use crate::PageSize;

use crate::util::str_to_bool;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageTablesEntries {
    pub source_pd: String,
    pub table_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageTableCopies {
    pub setvar: String,
    pub entries: Vec<PageTablesEntries>,
}

use zerocopy::{Immutable, IntoBytes};

impl PageTableCopies {
    pub fn from_xml(
        xml_sdf: &XmlSystemDescription,
        node: &dyn SdfNode,
    ) -> Result<PageTableCopies, String> {
        check_attributes(xml_sdf, node, &["setvar"])?;
        let setvar = checked_lookup(xml_sdf, node, "setvar")?.to_string();
        let mut pds = Vec::new();
        for child in node.children() {
            match child.tag_name() {
                "pd" => {
                    check_attributes(xml_sdf, &*child, &["name", "index"])?;
                    let source_pd = checked_lookup(xml_sdf, &*child, "name")?.to_string();
                    let index =
                        sdf_parse_number(checked_lookup(xml_sdf, &*child, "index")?, &*child)?;
                    let page_table_copy = PageTablesEntries {
                        source_pd,
                        table_index: index as usize,
                    };
                    pds.push(page_table_copy);
                }
                _ => {
                    return Err(format!(
                        "Invalid XML element '{}' in page_table",
                        child.tag_name(),
                    ));
                }
            }
        }

        Ok(PageTableCopies {
            setvar,
            entries: pds,
        })
    }
}
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
        let mut offset_table: [u64; PAGE_TABLE_ENTRIES as usize] =
            [u64::MAX; PAGE_TABLE_ENTRIES as usize];
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
        let pgd_index = ((vaddr & (PAGE_TABLE_MASK << PageTableMaskShift::PGD as u64))
            >> PageTableMaskShift::PGD as u64) as usize;
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

impl Default for PUD {
    fn default() -> Self {
        Self::new()
    }
}

impl PUD {
    pub fn new() -> Self {
        PUD {
            dirs: vec![None; PAGE_TABLE_ENTRIES as usize],
        }
    }

    pub fn recurse(&mut self, mut curr_offset: u64, buffer: &mut Vec<u8>) -> u64 {
        let mut offset_table: [u64; PAGE_TABLE_ENTRIES as usize] =
            [u64::MAX; PAGE_TABLE_ENTRIES as usize];
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
        let pud_index = ((vaddr & (PAGE_TABLE_MASK << PageTableMaskShift::PUD as u64))
            >> PageTableMaskShift::PUD as u64) as usize;
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
        let mut offset_table: [u64; PAGE_TABLE_ENTRIES as usize] =
            [u64::MAX; PAGE_TABLE_ENTRIES as usize];
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
        let dir_index = ((vaddr & (PAGE_TABLE_MASK << PageTableMaskShift::PD as u64))
            >> PageTableMaskShift::PD as u64) as usize;
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
        } else if size == PageSize::Large as u64 {
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
        let pt_index = ((vaddr & (PAGE_TABLE_MASK << PageTableMaskShift::PT as u64))
            >> PageTableMaskShift::PT as u64) as usize;
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
    Aarch64 { top_level: PGD },
}

