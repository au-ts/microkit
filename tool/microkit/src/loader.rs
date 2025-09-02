//
// Copyright 2024, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use crate::elf::ElfFile;
use crate::sel4::{Arch, Config};
use crate::util::{kb, mask, mb, round_up, struct_to_bytes};
use crate::MemoryRegion;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::mem;
use std::path::Path;
use std::slice;

const PAGE_TABLE_SIZE: usize = 4096;

const AARCH64_1GB_BLOCK_BITS: u64 = 30;
const AARCH64_2MB_BLOCK_BITS: u64 = 21;

const AARCH64_LVL0_BITS: u64 = 9;
const AARCH64_LVL1_BITS: u64 = 9;
const AARCH64_LVL2_BITS: u64 = 9;

struct Aarch64;
impl Aarch64 {
    pub fn lvl0_index(addr: u64) -> usize {
        let idx = (addr >> (AARCH64_2MB_BLOCK_BITS + AARCH64_LVL2_BITS + AARCH64_LVL1_BITS))
            & mask(AARCH64_LVL0_BITS);
        idx as usize
    }

    pub fn lvl1_index(addr: u64) -> usize {
        let idx = (addr >> (AARCH64_2MB_BLOCK_BITS + AARCH64_LVL2_BITS)) & mask(AARCH64_LVL1_BITS);
        idx as usize
    }

    pub fn lvl2_index(addr: u64) -> usize {
        let idx = (addr >> (AARCH64_2MB_BLOCK_BITS)) & mask(AARCH64_LVL2_BITS);
        idx as usize
    }
}

struct Riscv64;
impl Riscv64 {
    const BLOCK_BITS_2MB: u64 = 21;

    const PAGE_TABLE_INDEX_BITS: u64 = 9;
    const PAGE_SHIFT: u64 = 12;
    /// This sets the page table entry bits: D,A,X,W,R.
    const PTE_TYPE_BITS: u64 = 0b11001110;
    // TODO: where does this come from?
    const PTE_TYPE_TABLE: u64 = 0;
    const PTE_TYPE_VALID: u64 = 1;

    const PTE_PPN0_SHIFT: u64 = 10;

    /// Due to RISC-V having various virtual memory setups, we have this generic function to
    /// figure out the page-table index given the total number of page table levels for the
    /// platform and which level we are currently looking at.
    pub fn pt_index(pt_levels: usize, addr: u64, level: usize) -> usize {
        let pt_index_bits = Self::PAGE_TABLE_INDEX_BITS * (pt_levels - level) as u64;
        let idx = (addr >> (pt_index_bits + Self::PAGE_SHIFT)) % 512;

        idx as usize
    }

    /// Generate physical page number given an address
    pub fn pte_ppn(addr: u64) -> u64 {
        (addr >> Self::PAGE_SHIFT) << Self::PTE_PPN0_SHIFT
    }

    pub fn pte_next(addr: u64) -> u64 {
        Self::pte_ppn(addr) | Self::PTE_TYPE_TABLE | Self::PTE_TYPE_VALID
    }

    pub fn pte_leaf(addr: u64) -> u64 {
        Self::pte_ppn(addr) | Self::PTE_TYPE_BITS | Self::PTE_TYPE_VALID
    }
}

/// Checks that each region in the given list does not overlap with any other region.
/// Panics upon finding an overlapping region
fn check_non_overlapping(regions: &Vec<(u64, &[u8])>) {
    let mut checked: Vec<(u64, u64)> = Vec::new();
    for (base, data) in regions {
        let end = base + data.len() as u64;
        // Check that this does not overlap with any checked regions
        for (b, e) in &checked {
            if !(end <= *b || *base >= *e) {
                panic!("Overlapping regions: [{base:x}..{end:x}) overlaps [{b:x}..{e:x})");
            }
        }

        checked.push((*base, end));
    }
}

#[repr(C)]
struct LoaderRegion64 {
    load_addr: u64,
    size: u64,
    offset: u64,
    r#type: u64,
}

#[repr(C)]
struct LoaderKernelInfo64 {
    kernel_entry: u64,
    ui_p_reg_start: u64,
    ui_p_reg_end: u64,
    pv_offset: u64,
    v_entry: u64,
    extra_device_addr_p: u64,
    extra_device_size: u64,
}

#[repr(C)]
struct LoaderHeader64 {
    magic: u64,
    size: u64,
    flags: u64,
    num_multikernels: u64,
    num_regions: u64,
}

pub struct Loader<'a> {
    image: Vec<u8>,
    header: LoaderHeader64,
    kernel_data: Vec<LoaderKernelInfo64>,
    region_metadata: Vec<LoaderRegion64>,
    regions: Vec<(u64, &'a [u8])>,
    _additional_headers: Vec<LoaderHeader64>,
}

impl<'a> Loader<'a> {
    pub fn new(
        config: &Config,
        loader_elf_path: &Path,
        kernel_elf: &'a ElfFile,
        kernel_elf_p_v_offset: u64,
        initial_task_elf: &'a ElfFile,
        initial_task_phys_base: Option<u64>,
        reserved_region: MemoryRegion,
        system_regions: Vec<(u64, &'a [u8])>,
    ) -> Loader<'a> {
        // Note: If initial_task_phys_base is not None, then it just this address
        // as the base physical address of the initial task, rather than the address
        // that comes from the initial_task_elf file.
        let elf = ElfFile::from_path(loader_elf_path).unwrap();
        let sz = elf.word_size;
        let magic = match sz {
            32 => 0x5e14dead,
            64 => 0x5e14dead14de5ead,
            _ => panic!(
                "Internal error: unexpected ELF word size: {} from '{}'",
                sz,
                loader_elf_path.display()
            ),
        };

        let mut regions = Vec::new();

        let loadable_kernel_segments: Vec<_> = kernel_elf.loadable_segments();

        let kernel_first_vaddr = loadable_kernel_segments
            .first()
            .expect("kernel has at least one loadable segment")
            .virt_addr;

        let kernel_first_paddr = kernel_first_vaddr - kernel_elf_p_v_offset;

        for segment in loadable_kernel_segments {
            let region_paddr = segment.virt_addr - kernel_elf_p_v_offset;
            regions.push((region_paddr, segment.data.as_slice()));
        }

        // Note: This could be extended to support multi-segment ELF files
        // (and indeed initial did support multi-segment ELF files). However
        // it adds significant complexity, and the calling functions enforce
        // only single-segment ELF files, so we keep things simple here.
        let initial_task_segments: Vec<_> = initial_task_elf
            .segments
            .iter()
            .filter(|s| s.loadable)
            .collect();
        assert!(initial_task_segments.len() == 1);
        let segment = &initial_task_segments[0];
        assert!(segment.loadable);

        let inittask_first_vaddr = segment.virt_addr;
        let inittask_last_vaddr = round_up(segment.virt_addr + segment.mem_size(), kb(4));

        let inittask_first_paddr = match initial_task_phys_base {
            Some(paddr) => paddr,
            None => segment.phys_addr,
        };
        let inittask_p_v_offset = inittask_first_vaddr - inittask_first_paddr;

        // Note: For now we include any zeroes. We could optimize in the future
        regions.push((inittask_first_paddr, &segment.data));

        // Determine how many multikernels to load from the #defined value in loader.c
        println!("Extracting multikernel address");
        let (num_multikernels_addr, num_multikernels_size) = elf
            .find_symbol("num_multikernels")
            .expect("Could not find 'num_multikernels' symbol");

        println!("Reading multikernel number at {:x}", num_multikernels_addr);
        let num_multikernels: u64 = (*(elf
            .get_data(num_multikernels_addr, num_multikernels_size)
            .expect("Could not extract number of multikernels to boot"))
        .first()
        .expect("Failed to copy in number of multikernels to boot"))
        .into();
        println!("Recieved number {}", num_multikernels);
        assert!(num_multikernels > 0);

        // Debugging, delete later
        if num_multikernels > 1 {
            println!("MULTIKERNEL MODE ACTIVATED, number is {}", num_multikernels);
        } else {
            println!("MULTIKERNEL INACTIVE");
        }

        println!("Making pagetables");
        let pagetable_vars = match config.arch {
            Arch::Aarch64 => Loader::aarch64_setup_pagetables(
                &elf,
                kernel_first_vaddr,
                kernel_first_paddr,
                num_multikernels.try_into().unwrap(),
            ),
            Arch::Riscv64 => Loader::riscv64_setup_pagetables(
                config,
                &elf,
                kernel_first_vaddr,
                kernel_first_paddr,
                num_multikernels.try_into().unwrap(),
            ),
        };
        println!("Made pagetables");

        let image_segment = elf
            .segments
            .into_iter()
            .find(|segment| segment.loadable)
            .expect("Did not find loadable segment");
        let image_vaddr = image_segment.virt_addr;
        let mut image = image_segment.data;

        println!("Loader elf entry is {:x}", image_vaddr);

        if image_vaddr != elf.entry {
            panic!("The loader entry point must be the first byte in the image");
        }

        // Copy in all the page tables, for each level of the pagetable and then for each kernel
        println!("Writing pagetables to image..");
        let mut id: usize = 0;
        while id < num_multikernels as usize {
            for (var_addr, var_size, var_data) in &pagetable_vars {
                //println!("Pagetable id {} var_size is {} and var_data.len() is {}", id, var_size / (num_multikernels as u64), (var_data[id].len() as u64));
                let offset = var_addr - image_vaddr;
                assert!(var_size / (num_multikernels) == var_data[id].len() as u64);
                assert!(offset > 0);
                assert!(offset <= image.len() as u64);
                println!(
                    "Copying into the image at {:x} til {:x}",
                    offset as usize + (id * PAGE_TABLE_SIZE),
                    (offset + (var_size / (num_multikernels))) as usize + (id * PAGE_TABLE_SIZE)
                );
                image[offset as usize + (id * PAGE_TABLE_SIZE)
                    ..(offset + (var_size / (num_multikernels))) as usize + (id * PAGE_TABLE_SIZE)]
                    .copy_from_slice(&var_data[id]);
            }
            id += 1;
        }

        let mut kernel_entries = vec![kernel_elf.entry];
        for _ in 0..num_multikernels {
            kernel_entries.push(kernel_elf.entry);
        }

        let pv_offset = inittask_first_paddr.wrapping_sub(inittask_first_vaddr);

        let ui_p_reg_start = inittask_first_paddr;
        let ui_p_reg_end = inittask_last_vaddr - inittask_p_v_offset;
        assert!(ui_p_reg_end > ui_p_reg_start);

        let v_entry = initial_task_elf.entry;

        let extra_device_addr_p = reserved_region.base;
        let extra_device_size = reserved_region.size();

        println!(
            "There are {} regions and {} system regions",
            regions.len(),
            system_regions.len()
        );
        let mut all_regions = Vec::with_capacity(regions.len() + system_regions.len());
        for region_set in [regions, system_regions] {
            for r in region_set {
                all_regions.push(r);
            }
        }

        let mut all_regions_with_loader = all_regions.clone();
        println!("Image vaddr at: {:x}", image_vaddr);
        all_regions_with_loader.push((image_vaddr, &image));
        check_non_overlapping(&all_regions_with_loader);

        let flags = match config.hypervisor {
            true => 1,
            false => 0,
        };

        let mut region_metadata = Vec::new();
        let mut offset: u64 = 0;
        let mut last_addr: u64 = 0;
        let mut last_size: u64 = 0;
        for (addr, data) in &all_regions {
            println!(
                "Adding region at {:x} size {:x} and offset {:x}",
                *addr,
                data.len() as u64,
                offset
            );
            region_metadata.push(LoaderRegion64 {
                load_addr: *addr,
                size: data.len() as u64,
                offset,
                r#type: 1,
            });
            offset += data.len() as u64;

            if *addr > last_addr {
                last_addr = *addr;
                last_size = data.len() as u64;
            }
        }
        // Assuming regions are packed together and start at load addr 0x0......
        //let offset_size: u64 = ((last_addr + last_size) + 0xFFF) & !(0xFFF);
        let offset_size = 0x1000000;
        println!(
            "We can start adding from {:x} ({:x} + {:x} = {:x})",
            offset_size,
            last_addr,
            last_size,
            last_addr + last_size
        );
        // Once region meta data is finalised, add all regions again but with addresses that are offset by the last free addr
        //
        // So for each region in the list, add it 1..num_multikernel times
        // Then same offset etc, but each load addr is now addr + total_size * i
        let original_num_regions = region_metadata.len();
        println!("We have {} regions", original_num_regions);
        for i in 2..region_metadata.len() {
            // Change 2 to be num_multikernels
            for j in 1..num_multikernels {
                region_metadata.push(LoaderRegion64 {
                    load_addr: region_metadata[i].load_addr,
                    size: region_metadata[i].size,
                    offset: region_metadata[i].offset,
                    r#type: region_metadata[i].r#type,
                });
            }
        }
        println!(
            "We now have {} regions, expected {}",
            region_metadata.len(),
            original_num_regions * num_multikernels as usize
        );
        //assert!(region_metadata.len() == original_num_regions * num_multikernels as usize);

        for i in 0..num_multikernels {
            println!("-------------------");
            println!("    HEADER INFO    ");
            println!("-------------------");
            println!("kernel_entry: {:x}", kernel_entries[i as usize]);
            println!(
                "ui_p_reg_start: {:x} (user image physical start address)",
                ui_p_reg_start
            );
            println!(
                "ui_p_reg_end: {:x} (user image physical end address)",
                ui_p_reg_end
            );
            println!("pv_offset: {:x} (physical/virtual offset)", pv_offset);
            println!(
                "initial_task_elf entry: {:x}  (user image virtual entry address)",
                v_entry
            );
            println!("extra_device_addr_p: {:x}", extra_device_addr_p);
            println!("extra_device_size: {:x}", extra_device_size);
            println!("-------------------");
        }

        // Make new vector
        let mut kernel_data = Vec::new();
        for i in 0..num_multikernels {
            kernel_data.push(LoaderKernelInfo64 {
                kernel_entry: kernel_entries[i as usize],
                ui_p_reg_start: ui_p_reg_start,
                ui_p_reg_end: ui_p_reg_end,
                pv_offset: pv_offset,
                v_entry: v_entry,
                extra_device_addr_p: extra_device_addr_p,
                extra_device_size: extra_device_size,
            });
        }
        println!(
            "Kernel data was copied {} times (target {})",
            kernel_data.len(),
            num_multikernels
        );
        assert!(kernel_data.len() == num_multikernels as usize);
        // Copy header info to it like 4 times lmbao

        let size = std::mem::size_of::<LoaderHeader64>() as u64
            + region_metadata.iter().fold(0_u64, |acc, x| {
                acc + x.size + std::mem::size_of::<LoaderRegion64>() as u64
            });

        let header = LoaderHeader64 {
            magic,
            size,
            flags,
            num_multikernels,
            num_regions: region_metadata.len() as u64,
        };

        let mut additional_headers: Vec<LoaderHeader64> = Vec::new();
        additional_headers.push(LoaderHeader64 {
            magic,
            size,
            flags,
            num_multikernels,
            num_regions: region_metadata.len() as u64,
        });

        Loader {
            image,
            header,
            kernel_data,
            region_metadata,
            regions: all_regions,
            _additional_headers: additional_headers,
        }
    }

    pub fn write_image(&self, path: &Path) {
        let loader_file = match File::create(path) {
            Ok(file) => file,
            Err(e) => panic!("Could not create '{}': {}", path.display(), e),
        };

        let mut loader_buf = BufWriter::new(loader_file);

        // First write out all the image data
        println!("Writing image data");
        loader_buf
            .write_all(self.image.as_slice())
            .expect("Failed to write image data to loader");

        // Then we write out the loader metadata (known as the 'header')
        let header_bytes = unsafe { struct_to_bytes(&self.header) };
        loader_buf
            .write_all(header_bytes)
            .expect("Failed to write header data to loader");

        // Then kernel info bytes
        let kernel_bytes = unsafe {
            slice::from_raw_parts(
                self.kernel_data.as_ptr() as *const u8,
                self.kernel_data.len() * mem::size_of::<LoaderKernelInfo64>(),
            )
        };
        loader_buf
            .write_all(kernel_bytes)
            .expect("Failed to write kernel data");

        // For each region, we need to write out the region metadata as well
        for region in &self.region_metadata {
            let region_metadata_bytes = unsafe { struct_to_bytes(region) };
            loader_buf
                .write_all(region_metadata_bytes)
                .expect("Failed to write region metadata to loader");
        }

        // Now we can write out all the region data
        for (_, data) in &self.regions {
            loader_buf
                .write_all(data)
                .expect("Failed to write region data to loader");
        }

        loader_buf.flush().unwrap();
    }

    fn riscv64_setup_pagetables(
        config: &Config,
        elf: &ElfFile,
        first_vaddr: u64,
        first_paddr: u64,
        num_multikernels: usize,
    ) -> Vec<(u64, u64, Vec<[u8; PAGE_TABLE_SIZE]>)> {
        assert!(
            num_multikernels == 1,
            "Multikernel support for risc-v is not implemented."
        );

        let (text_addr, _) = elf
            .find_symbol("_text")
            .expect("Could not find 'text' symbol");
        let (boot_lvl1_pt_addr, boot_lvl1_pt_size) = elf
            .find_symbol("boot_lvl1_pt")
            .expect("Could not find 'boot_lvl1_pt' symbol");
        let (boot_lvl2_pt_addr, boot_lvl2_pt_size) = elf
            .find_symbol("boot_lvl2_pt")
            .expect("Could not find 'boot_lvl2_pt' symbol");
        let (boot_lvl2_pt_elf_addr, boot_lvl2_pt_elf_size) = elf
            .find_symbol("boot_lvl2_pt_elf")
            .expect("Could not find 'boot_lvl2_pt_elf' symbol");

        let num_pt_levels = config.riscv_pt_levels.unwrap().levels();

        let mut boot_lvl1_pt: Vec<[u8; PAGE_TABLE_SIZE]> =
            vec![[0; PAGE_TABLE_SIZE]; num_multikernels];
        {
            let text_index_lvl1 = Riscv64::pt_index(num_pt_levels, text_addr, 1);
            let pt_entry = Riscv64::pte_next(boot_lvl2_pt_elf_addr);
            let start = 8 * text_index_lvl1;
            let end = start + 8;
            boot_lvl1_pt[0][start..end].copy_from_slice(&pt_entry.to_le_bytes());
        }

        let mut boot_lvl2_pt_elf: Vec<[u8; PAGE_TABLE_SIZE]> =
            vec![[0; PAGE_TABLE_SIZE]; num_multikernels];
        {
            let text_index_lvl2 = Riscv64::pt_index(num_pt_levels, text_addr, 2);
            for (page, i) in (text_index_lvl2..512).enumerate() {
                let start = 8 * i;
                let end = start + 8;
                let addr = text_addr + ((page as u64) << Riscv64::BLOCK_BITS_2MB);
                let pt_entry = Riscv64::pte_leaf(addr);
                boot_lvl2_pt_elf[0][start..end].copy_from_slice(&pt_entry.to_le_bytes());
            }
        }

        {
            let index = Riscv64::pt_index(num_pt_levels, first_vaddr, 1);
            let start = 8 * index;
            let end = start + 8;
            boot_lvl1_pt[0][start..end]
                .copy_from_slice(&Riscv64::pte_next(boot_lvl2_pt_addr).to_le_bytes());
        }

        let mut boot_lvl2_pt: Vec<[u8; PAGE_TABLE_SIZE]> =
            vec![[0; PAGE_TABLE_SIZE]; num_multikernels];

        {
            let index = Riscv64::pt_index(num_pt_levels, first_vaddr, 2);
            for (page, i) in (index..512).enumerate() {
                let start = 8 * i;
                let end = start + 8;
                let addr = first_paddr + ((page as u64) << Riscv64::BLOCK_BITS_2MB);
                let pt_entry = Riscv64::pte_leaf(addr);
                boot_lvl2_pt[0][start..end].copy_from_slice(&pt_entry.to_le_bytes());
            }
        }

        vec![
            (boot_lvl1_pt_addr, boot_lvl1_pt_size, boot_lvl1_pt),
            (boot_lvl2_pt_addr, boot_lvl2_pt_size, boot_lvl2_pt),
            (
                boot_lvl2_pt_elf_addr,
                boot_lvl2_pt_elf_size,
                boot_lvl2_pt_elf,
            ),
        ]
    }

    fn aarch64_setup_pagetables(
        elf: &ElfFile,
        first_vaddr: u64,
        first_paddr: u64,
        num_multikernels: usize,
    ) -> Vec<(u64, u64, Vec<[u8; PAGE_TABLE_SIZE]>)> {
        let (boot_lvl1_lower_addr, boot_lvl1_lower_size) = elf
            .find_symbol("boot_lvl1_lower")
            .expect("Could not find 'boot_lvl1_lower' symbol");
        let (boot_lvl1_upper_addr, boot_lvl1_upper_size) = elf
            .find_symbol("boot_lvl1_upper")
            .expect("Could not find 'boot_lvl1_upper' symbol");
        let (boot_lvl2_upper_addr, boot_lvl2_upper_size) = elf
            .find_symbol("boot_lvl2_upper")
            .expect("Could not find 'boot_lvl2_upper' symbol");
        let (boot_lvl0_lower_addr, boot_lvl0_lower_size) = elf
            .find_symbol("boot_lvl0_lower")
            .expect("Could not find 'boot_lvl0_lower' symbol");
        let (boot_lvl0_upper_addr, boot_lvl0_upper_size) = elf
            .find_symbol("boot_lvl0_upper")
            .expect("Could not find 'boot_lvl0_upper' symbol");

        // Make table vectors
        let mut boot_lvl0_lower: Vec<[u8; PAGE_TABLE_SIZE]> =
            vec![[0; PAGE_TABLE_SIZE]; num_multikernels];
        let mut boot_lvl1_lower: Vec<[u8; PAGE_TABLE_SIZE]> =
            vec![[0; PAGE_TABLE_SIZE]; num_multikernels];
        let boot_lvl0_upper: Vec<[u8; PAGE_TABLE_SIZE]> =
            vec![[0; PAGE_TABLE_SIZE]; num_multikernels];
        let mut boot_lvl1_upper: Vec<[u8; PAGE_TABLE_SIZE]> =
            vec![[0; PAGE_TABLE_SIZE]; num_multikernels];
        let mut boot_lvl2_upper: Vec<[u8; PAGE_TABLE_SIZE]> =
            vec![[0; PAGE_TABLE_SIZE]; num_multikernels];

        // Populate all the page tables the same
        let mut id: usize = 0;
        while id < num_multikernels {
            //println!("Initing id {}", id);

            //println!("Making first level, size is {} with num kernels is {} and hence final size should be {}", PAGE_TABLE_SIZE, num_multikernels, PAGE_TABLE_SIZE * num_multikernels);
            boot_lvl0_lower[id][..8].copy_from_slice(&(boot_lvl1_lower_addr | 3).to_le_bytes());
            //println!("Made first level");

            for i in 0..512 {
                #[allow(clippy::identity_op)] // keep the (0 << 2) for clarity
                let pt_entry: u64 = ((i as u64) << AARCH64_1GB_BLOCK_BITS) |
                    (1 << 10) | // access flag
                    (0 << 2) | // strongly ordered memory
                    (1); // 1G block
                let start = 8 * i;
                let end = 8 * (i + 1);
                boot_lvl1_lower[id][start..end].copy_from_slice(&pt_entry.to_le_bytes());
            }

            {
                let pt_entry = (boot_lvl1_upper_addr | 3).to_le_bytes();
                let idx = Aarch64::lvl0_index(first_vaddr);
                boot_lvl0_lower[id][8 * idx..8 * (idx + 1)].copy_from_slice(&pt_entry);
            }

            {
                let pt_entry = (boot_lvl2_upper_addr | 3).to_le_bytes();
                let idx = Aarch64::lvl1_index(first_vaddr);
                boot_lvl1_upper[id][8 * idx..8 * (idx + 1)].copy_from_slice(&pt_entry);
            }

            let lvl2_idx = Aarch64::lvl2_index(first_vaddr);
            for i in lvl2_idx..512 {
                let entry_idx = (i - Aarch64::lvl2_index(first_vaddr)) << AARCH64_2MB_BLOCK_BITS;
                let pt_entry: u64 = (entry_idx as u64 + first_paddr) |
                    (1 << 10) | // Access flag
                    (3 << 8) | // Make sure the shareability is the same as the kernel's
                    (4 << 2) | // MT_NORMAL memory
                    (1 << 0); // 2MB block
                let start = 8 * i;
                let end = 8 * (i + 1);
                boot_lvl2_upper[id][start..end].copy_from_slice(&pt_entry.to_le_bytes());
            }

            id += 1;
        }

        vec![
            (boot_lvl0_lower_addr, boot_lvl0_lower_size, boot_lvl0_lower),
            (boot_lvl1_lower_addr, boot_lvl1_lower_size, boot_lvl1_lower),
            (boot_lvl0_upper_addr, boot_lvl0_upper_size, boot_lvl0_upper),
            (boot_lvl1_upper_addr, boot_lvl1_upper_size, boot_lvl1_upper),
            (boot_lvl2_upper_addr, boot_lvl2_upper_size, boot_lvl2_upper),
        ]
    }
}
