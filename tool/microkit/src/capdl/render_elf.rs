//
// Copyright 2023, Colias Group, LLC
//
// SPDX-License-Identifier: BSD-2-Clause
//

// A copy of https://github.com/seL4/rust-sel4/blob/6f8d1baaad3aaca6f20966a2acb40e4651546519/crates/sel4-capdl-initializer/add-spec/src/render_elf.rs
// With visibilities modified from `pub(crate)` to `pub`.

use num::NumCast;
use object::read::elf::FileHeader;

use sel4_synthetic_elf::{Builder, PatchValue, Segment, PF_W};

pub struct RenderElfArgs<'a> {
    pub data: &'a [u8],
    pub granule_size_bits: usize,
    pub heap_size: usize,
}

impl RenderElfArgs<'_> {
    pub fn call_with<T: FileHeader<Word: NumCast + PatchValue>>(
        &self,
        orig_elf: &object::read::elf::ElfFile<T>,
    ) -> Vec<u8> {
        let mut builder = Builder::new(orig_elf).unwrap();

        builder.discard_p_align(true);

        let granule_size_bytes = 1 << self.granule_size_bits;

        {
            let align_residue = (granule_size_bytes
                - u64::try_from(self.data.len()).unwrap() % granule_size_bytes)
                % granule_size_bytes;

            let vaddr = builder.next_vaddr().next_multiple_of(granule_size_bytes) + align_residue;

            builder.add_segment(Segment::simple(vaddr, self.data.into()));

            builder
                .patch_word_with_cast("sel4_capdl_initializer_serialized_spec_start", vaddr)
                .unwrap();
            builder
                .patch_word_with_cast(
                    "sel4_capdl_initializer_serialized_spec_size",
                    self.data.len(),
                )
                .unwrap();
        }

        {
            let vaddr = builder.next_vaddr().next_multiple_of(granule_size_bytes);

            builder.add_segment({
                let mut segment = Segment::simple(vaddr, vec![].into());
                segment.p_flags |= PF_W;
                segment.p_memsz = u64::try_from(self.heap_size).unwrap();
                segment
            });

            builder
                .patch_word_with_cast("sel4_capdl_initializer_heap_start", vaddr)
                .unwrap();
            builder
                .patch_word_with_cast("sel4_capdl_initializer_heap_size", self.heap_size)
                .unwrap();
        }

        builder
            .patch_word_with_cast(
                "sel4_capdl_initializer_image_start",
                builder.footprint().unwrap().start,
            )
            .unwrap();
        builder
            .patch_word_with_cast(
                "sel4_capdl_initializer_image_end",
                builder.footprint().unwrap().end,
            )
            .unwrap();

        builder.build().unwrap()
    }
}
