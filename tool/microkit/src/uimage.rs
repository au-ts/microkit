//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

// Document referenced:
// U-Boot: include/image.h

use crate::{crc32::crc32, sel4::Arch};

// OS-type code
const IH_OS_ELF: u8 = 29;

// CPU-arch codes
const IH_ARCH_ARM64: u8 = 22;
const IH_ARCH_RISCV: u8 = 26;

// Image-type code
const IH_TYPE_KERNEL: u8 = 2;

// No compression
const IH_COMP_NONE: u8 = 0;

// Image name length max
const IH_NMLEN: usize = 32;

// Image magic
const IH_MAGIC: u32 = 0x27051956;

#[repr(C, packed)]
struct LegacyImgHeader {
    ih_magic: u32,           // Image Header Magic Number
    ih_hcrc: u32,            // Image Header CRC Checksum
    ih_time: u32,            // Image Creation Timestamp
    ih_size: u32,            // Image Data Size
    ih_load: u32,            // Data Load Address
    ih_ep: u32,              // Entry Point Address
    ih_dcrc: u32,            // Image Data CRC Checksum
    ih_os: u8,               // Operating System
    ih_arch: u8,             // CPU architecture
    ih_type: u8,             // Image Type
    ih_comp: u8,             // Compression Type
    ih_name: [u8; IH_NMLEN], // Image Name
}

pub fn uimage_serialise(arch: &Arch, entry: u32, elf_payload: Vec<u8>, out: &std::path::Path) {
    let ih_arch_le = match arch {
        Arch::Aarch64 => IH_ARCH_ARM64,
        Arch::Riscv64 => IH_ARCH_RISCV,
        Arch::X86_64 => unreachable!("internal bug: cannot create a Uimage for x86"),
    };

    let mut hdr = LegacyImgHeader {
        ih_magic: IH_MAGIC.to_be(),
        ih_hcrc: 0, // U-Boot clears this field before it recalculate the checksum, so do the same here
        ih_time: 0,
        ih_size: (elf_payload.len() as u32).to_be(),
        ih_load: 0, // ignored by U-Boot for uncompressed payload
        ih_ep: entry.to_be(),
        ih_dcrc: crc32(&elf_payload).to_be(),
        ih_os: IH_OS_ELF.to_be(),
        ih_arch: ih_arch_le.to_be(),
        ih_type: IH_TYPE_KERNEL.to_be(),
        ih_comp: IH_COMP_NONE.to_be(),
        ih_name: [0; IH_NMLEN],
    };

    let hdr_chksum = unsafe { crc32(struct_to_bytes(hdr)) };
    hdr.ih_hcrc = hdr_chksum;

    let mut uimage_file = match File::create(out) {
        Ok(file) => file,
        Err(e) => return Err(format!("Uimage: cannot create '{}': {}", out.display(), e)),
    };

    uimage_file
        .write_all(unsafe {
            from_raw_parts(
                (&hdr as *const LegacyImgHeader) as *const u8,
                size_of::<LegacyImgHeader>(),
            )
        })
        .unwrap_or_else(|_| panic!("Failed to write Uimage header for '{}'", out.display()));

    
}
