//
// Copyright 2023, Colias Group, LLC
//
// SPDX-License-Identifier: BSD-2-Clause
//

// A reimplementation of https://github.com/seL4/rust-sel4/blob/6f8d1baaad3aaca6f20966a2acb40e4651546519/crates/sel4-capdl-initializer/add-spec/src/reserialize_spec.rs
// We can't reuse the original code as is because it assumes that we are loading ELF frames from files.
// Which isn't suitable for Microkit as we want to embed the frames' data directly into the spec for
// easily patching ELF symbols.

use std::ops::Range;

use sel4_capdl_initializer_types::*;

use crate::capdl::spec::BytesContent;

// Given a `Spec` data structure from sel4_capdl_initializer_types, "flatten" it into a vector of bytes
// for encapsulating it into the CapDL initialiser ELF.
// Note that `BytesContent` comes from our spec.rs rather than sel4_capdl_initializer_types's because
// it is not as easy to deserialise a JSON sequence into a `&'a [u8]` than a `Vec<u8>`
pub fn reserialize_spec<'a>(
    input_spec: &Spec<'static, String, BytesContent, ()>,
    object_names_level: &ObjectNamesLevel,
) -> Vec<u8> {
    // A data structure to manage allocation of buffers in the flattened spec.
    let mut sources = SourcesBuilder::new();

    let final_spec = input_spec
        // This first step applies the debugging level from `object_names_level` to all root object
        // and copy them into `sources`.
        .traverse_names_with_context(|named_obj| {
            object_names_level
                .apply(named_obj)
                .map(|s| IndirectObjectName {
                    range: sources.append(s.as_bytes()),
                })
        })
        // The final step is to take the frame data and compress it using miniz_oxide::deflate::compress_to_vec()
        // so to save memory then append it to `sources`.
        .traverse_data(|data| IndirectDeflatedBytesContent {
            deflated_bytes_range: sources.append(&DeflatedBytesContent::pack(&data.bytes)),
        });

    let mut blob = postcard::to_allocvec(&final_spec).unwrap();
    blob.extend(sources.build());
    blob
}

struct SourcesBuilder {
    buf: Vec<u8>,
}

impl SourcesBuilder {
    fn new() -> Self {
        Self { buf: vec![] }
    }

    fn build(self) -> Vec<u8> {
        self.buf
    }

    fn append(&mut self, bytes: &[u8]) -> Range<usize> {
        let start = self.buf.len();
        self.buf.extend(bytes);
        let end = self.buf.len();
        start..end
    }
}
