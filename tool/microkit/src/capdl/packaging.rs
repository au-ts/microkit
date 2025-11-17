//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use crate::{
    capdl::{initialiser::CapDLInitialiser, CapDLSpecContainer},
    elf::ElfFile,
    sel4::{Config, PageSize},
    capdl::spec::{ElfContent, ByteData, FrameData},
};

pub fn pack_spec_into_initial_task(
    sel4_config: &Config,
    build_config: &str,
    spec_container: &CapDLSpecContainer,
    system_elfs: &[ElfFile],
    capdl_initialiser: &mut CapDLInitialiser,
) {
    let compress_frame = true;

    let (mut output_spec, _) = spec_container.spec.embed_fill(
        PageSize::Small.fixed_size_bits(sel4_config) as u8,
        |_| false,
        |d, buf| {
            match d {
                FrameData::Elf(elf_content) => {
                    buf.copy_from_slice(
                        &system_elfs[elf_content.elf_id].segments[elf_content.elf_seg_idx].data()[elf_content.elf_seg_data_range.clone()],
                    );
                },
                FrameData::Bytes(byte_content) => {
                    buf.copy_from_slice(&byte_content.data);
                }
            }
           
            compress_frame
        },
    );

    for named_obj in output_spec.objects.iter_mut() {
        match build_config {
            "debug" => {}
            // We don't copy over the object names as there is no debug printing in these configuration to save memory.
            "release" | "benchmark" => named_obj.name = None,
            _ => panic!("unknown configuration {build_config}"),
        };
    }

    let initialiser_payload = output_spec.to_bytes().unwrap();

    if capdl_initialiser.have_spec() {
        capdl_initialiser.replace_spec(initialiser_payload);
    } else {
        capdl_initialiser.add_spec(initialiser_payload);
    }
}
