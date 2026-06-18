//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use std::ops::Range;

use rkyv::util::AlignedVec;

use crate::elf::{ElfProgramHeader64, ElfSegmentData, PHENT_TYPE_PHDR, PHENT_TYPE_LOADABLE};
use crate::util::{round_up, struct_to_bytes};
use crate::{elf::ElfFile, sel4::PageSize};
use crate::{serialise_ut, UntypedObject};

// Page size used for allocating the spec and embedded frames segments.
pub const INITIALISER_GRANULE_SIZE: PageSize = PageSize::Small;

// Magic numbers for the initialiser to identify the data type.
// See rust-sel4 crates/sel4-phdrs/constants/src/lib.rs
const PT_SEL4_CAPDL_SPEC: u32 = 0x64c3_4003;
const PT_SEL4_CAPDL_FRAME_DATA: u32 = 0x64c3_4004;

pub struct CapDLInitialiserSpecMetadata {
    pub spec_size: u64,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

pub struct CapDLInitialiser {
    pub elf: ElfFile,
    pub phys_base: Option<u64>,
    pub spec_metadata: Option<CapDLInitialiserSpecMetadata>,
    /// Log level of initialiser printing in debug mode.
    pub log_level: LogLevel,
}

impl CapDLInitialiser {
    pub fn new(elf: ElfFile) -> CapDLInitialiser {
        CapDLInitialiser {
            elf,
            phys_base: None,
            spec_metadata: None,
            log_level: LogLevel::Info,
        }
    }

    pub fn image_bound(&self) -> Range<u64> {
        self.elf.lowest_vaddr()..round_up(self.elf.highest_vaddr(), INITIALISER_GRANULE_SIZE as u64)
    }

    pub fn add_spec(&mut self, spec_payload: AlignedVec, embedded_frame_data: Vec<u8>) {
        if self.spec_metadata.is_some() {
            self.elf.segments.pop();
            self.elf.segments.pop();
            self.spec_metadata = None;
        }

        // Follow implementation in rust-sel4: crates/sel4-capdl-initializer/add-spec/src/lib.rs
        let spec_vaddr = self.elf.next_vaddr(INITIALISER_GRANULE_SIZE);
        let spec_size = spec_payload.len() as u64;
        self.elf.add_segment(
            true,
            false,
            false,
            spec_vaddr,
            ElfSegmentData::RealData(spec_payload.into()),
            Some(PT_SEL4_CAPDL_SPEC),
        );

        println!("spec vaddr {:x}..{:x}", spec_vaddr, spec_vaddr + spec_size);

        let embedded_frame_data_vaddr = self.elf.next_vaddr(INITIALISER_GRANULE_SIZE);

        println!(
            "frames vaddr {:x}..{:x}",
            embedded_frame_data_vaddr,
            embedded_frame_data_vaddr as usize + embedded_frame_data.len()
        );

        self.elf.add_segment(
            true,
            false,
            false,
            embedded_frame_data_vaddr,
            ElfSegmentData::RealData(embedded_frame_data.clone()),
            Some(PT_SEL4_CAPDL_FRAME_DATA),
        );

        // Follow implementation in rust-sel4: crates/sel4-patch-elf/src/lib.rs
        // See `pub fn finalize(mut self) -> Vec<u8>` of `impl<'a, T: FileHeaderExt> Patching<'a, T>`

        let prog_headers_table_vaddr = self.elf.next_vaddr(INITIALISER_GRANULE_SIZE);
        let prog_headers_table = self.elf.program_headers(1032);
        let mut prog_headers_table_bytes: Vec<u8> = vec![];
        for ph in prog_headers_table.iter() {
            prog_headers_table_bytes.extend(unsafe { struct_to_bytes(ph) });
        }
        prog_headers_table_bytes.extend(unsafe {
            struct_to_bytes(&ElfProgramHeader64 {
                type_: PHENT_TYPE_PHDR,
                flags: 1 | 2 | 4,
                offset: 0,
                vaddr: prog_headers_table_vaddr,
                paddr: prog_headers_table_vaddr,
                filesz: (prog_headers_table_bytes.len() + size_of::<ElfProgramHeader64>()) as u64,
                memsz: (prog_headers_table_bytes.len() + size_of::<ElfProgramHeader64>()) as u64,
                align: 0,
            })
        });
        prog_headers_table_bytes.extend(unsafe {
            struct_to_bytes(&ElfProgramHeader64 {
                type_: PHENT_TYPE_LOADABLE,
                flags: 1 | 2 | 4,
                offset: 0,
                vaddr: prog_headers_table_vaddr,
                paddr: prog_headers_table_vaddr,
                filesz: (prog_headers_table_bytes.len() + size_of::<ElfProgramHeader64>()) as u64,
                memsz: (prog_headers_table_bytes.len() + size_of::<ElfProgramHeader64>()) as u64,
                align: 0,
            })
        });
        prog_headers_table_bytes.extend(unsafe {
            struct_to_bytes(&ElfProgramHeader64 {
                type_: PHENT_TYPE_LOADABLE,
                flags: 1 | 2 | 4,
                offset: 0,
                vaddr: spec_vaddr,
                paddr: spec_vaddr,
                filesz: spec_size as u64,
                memsz: spec_size as u64,
                align: 0,
            })
        });
        prog_headers_table_bytes.extend(unsafe {
            struct_to_bytes(&ElfProgramHeader64 {
                type_: PHENT_TYPE_LOADABLE,
                flags: 1 | 2 | 4,
                offset: 0,
                vaddr: embedded_frame_data_vaddr,
                paddr: embedded_frame_data_vaddr,
                filesz: embedded_frame_data.len() as u64,
                memsz: embedded_frame_data.len() as u64,
                align: 0,
            })
        });

        println!(
            "ph vaddr {:x}..{:x}",
            prog_headers_table_vaddr,
            prog_headers_table_vaddr as usize + prog_headers_table_bytes.len()
        );

        self.elf.add_segment(
            true,
            false,
            false,
            prog_headers_table_vaddr,
            ElfSegmentData::RealData(prog_headers_table_bytes),
            Some(PHENT_TYPE_PHDR),
        );

        self.elf
            .write_symbol(
                "sel4_phdrs_patched__vaddr",
                &prog_headers_table_vaddr.to_le_bytes(),
            )
            .unwrap();

        self.elf
            .write_symbol(
                "sel4_phdrs_patched__phnum",
                &(prog_headers_table.len() as u16 + 4).to_le_bytes(),
            )
            .unwrap();

        self.spec_metadata = Some(CapDLInitialiserSpecMetadata { spec_size });
    }

    pub fn spec_metadata(&self) -> &Option<CapDLInitialiserSpecMetadata> {
        &self.spec_metadata
    }

    pub fn add_expected_untypeds(&mut self, untypeds: &[UntypedObject]) {
        let mut uts_desc: Vec<u8> = Vec::new();
        for ut in untypeds.iter() {
            uts_desc.extend(serialise_ut(ut));
        }

        // This feature is currently not in mainline rust-seL4, keep it around for potential
        // debugging purposes.
        if self
            .elf
            .find_symbol("sel4_capdl_initializer_expected_untypeds_list_num_entries")
            .is_ok()
        {
            self.elf
                .write_symbol(
                    "sel4_capdl_initializer_expected_untypeds_list_num_entries",
                    &(untypeds.len() as u64).to_le_bytes(),
                )
                .unwrap();
            self.elf
                .write_symbol("sel4_capdl_initializer_expected_untypeds_list", &uts_desc)
                .unwrap();
        }
    }

    pub fn set_phys_base(&mut self, phys_base: u64) {
        self.phys_base = Some(phys_base);
    }
}
