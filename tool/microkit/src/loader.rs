//
// Copyright 2024, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

use crate::elf::ElfFile;
use crate::kernel_bootinfo::{
    seL4_KernelBootInfo, seL4_KernelBoot_KernelRegion, seL4_KernelBoot_RamRegion,
    seL4_KernelBoot_ReservedRegion, seL4_KernelBoot_RootTaskRegion, SEL4_KERNEL_BOOT_INFO_MAGIC,
    SEL4_KERNEL_BOOT_INFO_VERSION_0,
};
use crate::sel4::{Arch, Config};
use crate::util::{kb, mask, round_up, struct_to_bytes};
use crate::MemoryRegion;
use std::fs::File;
use std::io::{BufWriter, Seek, Write};
use std::iter::zip;
use std::mem;
use std::path::Path;

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
fn check_non_overlapping(regions: &Vec<(u64, &[u8], String)>) {
    let mut checked: Vec<(u64, u64)> = Vec::new();
    for &(base, data, _) in regions {
        let end = base + data.len() as u64;
        // Check that this does not overlap with any checked regions
        for &(b, e) in checked.iter() {
            if !(end <= b || base >= e) {
                // XXX: internal error?
                eprintln!("Overlapping regions: [{base:x}..{end:x}) overlaps [{b:x}..{e:x})");

                for (i, &(base_i, data, ref name)) in regions.iter().enumerate() {
                    let end_i = base_i + data.len() as u64;
                    eprint!("{i:>4}: [{base_i:#x}..{end_i:#x}) {name:<20}");
                    if (base == base_i && end == end_i) || (b == base_i && e == end_i) {
                        eprintln!(" (overlapping)");
                    } else {
                        eprintln!();
                    }
                }

                std::process::exit(1);
            }
        }

        checked.push((base, end));
    }
}

#[repr(C)]
#[derive(Debug)]
struct LoaderRegion64 {
    load_addr: u64,
    size: u64,
    offset: u64,
    r#type: u64,
}

// struct loader_data
#[repr(C)]
struct LoaderHeader64 {
    magic: u64,
    size: u64,
    flags: u64,
    num_multikernels: u64,
    num_regions: u64,
    kernel_v_entry: u64,
}

pub struct Loader<'a> {
    image: Vec<u8>,
    header: LoaderHeader64,
    kernel_bootinfos: Vec<(
        seL4_KernelBootInfo,
        // This ordering matches the ordering required by seL4_KernelBootInfo.
        Vec<seL4_KernelBoot_KernelRegion>,
        Vec<seL4_KernelBoot_RamRegion>,
        Vec<seL4_KernelBoot_RootTaskRegion>,
        Vec<seL4_KernelBoot_ReservedRegion>,
    )>,
    region_metadata: Vec<LoaderRegion64>,
    regions: Vec<(u64, &'a [u8], String)>,
}

impl<'a> Loader<'a> {
    pub fn new(
        config: &Config,
        loader_elf_path: &Path,
        kernel_elf: &'a ElfFile,
        kernel_elf_pv_offsets: &[u64],
        initial_task_elfs: &'a [ElfFile],
        initial_task_phys_base: &[u64],
        reserved_regions: &[MemoryRegion],
        system_regions: Vec<(u64, &'a [u8])>,
        per_core_ram_regions: &[&[MemoryRegion]],
    ) -> Loader<'a> {
        // Note: If initial_task_phys_base is not None, then it just this address
        // as the base physical address of the initial task, rather than the address
        // that comes from the initial_task_elf file.
        let loader_elf = ElfFile::from_path(loader_elf_path).unwrap();
        let sz = loader_elf.word_size;
        let magic = match sz {
            32 => 0x5e14dead,
            64 => 0x5e14dead14de5ead,
            _ => panic!(
                "Internal error: unexpected ELF word size: {} from '{}'",
                sz,
                loader_elf_path.display()
            ),
        };

        // Determine how many multikernels to load from the #defined value in loader.c
        println!("Extracting multikernel address");
        let (num_multikernels_addr, num_multikernels_size) = loader_elf
            .find_symbol("num_multikernels")
            .expect("Could not find 'num_multikernels' symbol");

        println!("Reading multikernel number at {:x}", num_multikernels_addr);
        let num_multikernels: usize = (*(loader_elf
            .get_data(num_multikernels_addr, num_multikernels_size)
            .expect("Could not extract number of multikernels to boot"))
        .first()
        .expect("Failed to copy in number of multikernels to boot"))
        .into();
        println!("Recieved number {}", num_multikernels);
        assert!(num_multikernels > 0);

        let mut kernel_regions = Vec::new();
        let mut inittask_regions: Vec<(u64, &'a [u8])> = Vec::new();

        // Delete it.
        #[allow(unused_variables)]
        let kernel_elf_p_v_offset = ();

        let loadable_kernel_segments: Vec<_> = kernel_elf.loadable_segments();
        let kernel_first_vaddr = loadable_kernel_segments
            .first()
            .expect("kernel has at least one loadable segment")
            .virt_addr;

        let mut kernel_first_paddrs = vec![];
        for kernel_p_v_offset in kernel_elf_pv_offsets {
            let kernel_first_paddr = kernel_first_vaddr - kernel_p_v_offset;
            kernel_first_paddrs.push(kernel_first_paddr);

            for segment in &loadable_kernel_segments {
                let region_paddr = segment.virt_addr - kernel_p_v_offset;
                kernel_regions.push((region_paddr, segment.data.as_slice()));
            }
        }

        // Remove mut.
        let kernel_first_paddrs = kernel_first_paddrs;
        println!("{:x?}", kernel_first_paddrs);

        // Note: This could be extended to support multi-segment ELF files
        // (and indeed initial did support multi-segment ELF files). However
        // it adds significant complexity, and the calling functions enforce
        // only single-segment ELF files, so we keep things simple here.
        let mut initial_task_info = vec![];
        for multikernel_idx in 0..num_multikernels {
            let initial_task_elf = &initial_task_elfs[multikernel_idx];
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

            let inittask_first_paddr = initial_task_phys_base[multikernel_idx];
            let inittask_p_v_offset = inittask_first_vaddr.wrapping_sub(inittask_first_paddr);

            // Note: For now we include any zeroes. We could optimize in the future
            inittask_regions.push((inittask_first_paddr, &segment.data));

            let pv_offset = inittask_first_paddr.wrapping_sub(inittask_first_vaddr);

            let ui_p_reg_start = inittask_first_paddr;
            let ui_p_reg_end = inittask_last_vaddr.wrapping_sub(inittask_p_v_offset);
            assert!(ui_p_reg_end > ui_p_reg_start);

            let v_entry = initial_task_elf.entry;

            initial_task_info.push((pv_offset, ui_p_reg_start, ui_p_reg_end, v_entry))
        }

        println!("Making pagetables");
        let pagetable_vars: Vec<_> = kernel_first_paddrs
            .iter()
            .enumerate()
            .map(|(i, paddr)| match config.arch {
                Arch::Aarch64 => Loader::aarch64_setup_pagetables(
                    &loader_elf,
                    kernel_first_vaddr,
                    *paddr,
                    (i * PAGE_TABLE_SIZE) as u64,
                ),
                Arch::Riscv64 => Loader::riscv64_setup_pagetables(
                    config,
                    &loader_elf,
                    kernel_first_vaddr,
                    *paddr,
                ),
            })
            .collect();
        println!("Made pagetables");

        let image_segment = loader_elf
            .segments
            .into_iter()
            .find(|segment| segment.loadable)
            .expect("Did not find loadable segment");
        let image_vaddr = image_segment.virt_addr;
        let mut image = image_segment.data;

        println!("Loader elf entry is {:x}", image_vaddr);

        if image_vaddr != loader_elf.entry {
            panic!("The loader entry point must be the first byte in the image");
        }

        // Copy in all the page tables, for each level of the pagetable and then for each kernel
        println!("Writing pagetables to image..");
        for id in 0..num_multikernels {
            for (var_addr, var_size, var_data) in &pagetable_vars[id] {
                //println!("Pagetable id {} var_size is {} and var_data.len() is {}", id, var_size / (num_multikernels as u64), (var_data[id].len() as u64));
                let offset = var_addr - image_vaddr;
                let var_size = var_size / (num_multikernels as u64);
                assert!(var_size == var_data.len() as u64);
                assert!(offset > 0);
                assert!(offset <= image.len() as u64);
                println!(
                    "Copying into the image at {:x} til {:x}",
                    offset,
                    (offset + var_size) as usize
                );
                println!("sum: {}", var_data.iter().map(|v| *v as u64).sum::<u64>());
                image[offset as usize..(offset + var_size) as usize].copy_from_slice(var_data);
            }
        }

        println!(
            "There are {} inittask regions and {} kernel regions and {} system regions",
            inittask_regions.len(),
            kernel_regions.len(),
            system_regions.len()
        );
        let mut all_regions: Vec<(u64, &[u8], String)> = Vec::with_capacity(
            inittask_regions.len() + system_regions.len() + kernel_regions.len(),
        );
        all_regions.extend(
            kernel_regions
                .iter()
                .enumerate()
                .map(|(i, kr)| (kr.0, kr.1, format!("kernel {i}"))),
        );
        for &(base, data) in inittask_regions.iter() {
            all_regions.push((base, data, format!("Initial task region")));
        }
        for (i, &(base, data)) in system_regions.iter().enumerate() {
            all_regions.push((base, data, format!("System region {i}")));
        }

        let mut all_regions_with_loader = all_regions.clone();
        println!("Loader image vaddr at: {:x}", image_vaddr);
        all_regions_with_loader.push((image_vaddr, &image, format!("Loader")));
        check_non_overlapping(&all_regions_with_loader);

        let flags = match config.hypervisor {
            true => 1,
            false => 0,
        };

        let mut region_metadata = Vec::new();
        let mut offset: u64 = 0;
        for (addr, data, _) in &all_regions {
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
        }

        let mut kernel_bootinfos = Vec::new();
        for (
            (pv_offset, ui_p_reg_start, ui_p_reg_end, root_task_entry),
            (&raw_ram_regions, (extra_device_region, kernel_first_paddr)),
        ) in zip(
            initial_task_info,
            zip(
                per_core_ram_regions,
                zip(reserved_regions, kernel_first_paddrs.iter()),
            ),
        ) {
            let kernel_regions = vec![seL4_KernelBoot_KernelRegion {
                base: *kernel_first_paddr,
                // TODO
                end: 0,
            }];
            let ram_regions = raw_ram_regions
                .iter()
                .map(|r| seL4_KernelBoot_RamRegion {
                    base: r.base,
                    end: r.end,
                })
                .collect::<Vec<_>>();
            let root_task_regions = vec![
                // TODO: remove pv_offset
                seL4_KernelBoot_RootTaskRegion {
                    paddr_base: ui_p_reg_start,
                    paddr_end: ui_p_reg_end,
                    vaddr_base: ui_p_reg_start.wrapping_sub(pv_offset),
                    _padding: [0; 8],
                },
            ];
            let mut reserved_regions = vec![seL4_KernelBoot_ReservedRegion {
                base: extra_device_region.base,
                end: extra_device_region.end,
            }];
            // "Normal memory" and "kernel memory" (usually a subset of normal), i.e. "RAM"
            // available to each core must be reserved so that kernels don't rely on
            // memory available to other (e.g. for kernel-internal structures)
            for other_core_ram in per_core_ram_regions
                .iter()
                .filter(|&&regions| regions != raw_ram_regions)
            {
                for region in other_core_ram.iter() {
                    reserved_regions.push(seL4_KernelBoot_ReservedRegion {
                        base: region.base,
                        end: region.end,
                    });
                }
            }
            assert!(reserved_regions.len() == 1 + per_core_ram_regions.len() - 1);

            let info = seL4_KernelBootInfo {
                magic: SEL4_KERNEL_BOOT_INFO_MAGIC,
                version: SEL4_KERNEL_BOOT_INFO_VERSION_0,
                _padding0: [0; 3],
                root_task_entry,
                num_kernel_regions: kernel_regions
                    .len()
                    .try_into()
                    .expect("cannot fit # kernel regions into u8"),
                num_ram_regions: ram_regions
                    .len()
                    .try_into()
                    .expect("cannot fit # ram regions into u8"),
                num_root_task_regions: root_task_regions
                    .len()
                    .try_into()
                    .expect("cannot fit # root task regions into u8"),
                num_reserved_regions: reserved_regions
                    .len()
                    .try_into()
                    .expect("cannot fit # reserved regions into u8"),
                _padding: [0; 4],
            };

            kernel_bootinfos.push((
                info,
                kernel_regions,
                ram_regions,
                root_task_regions,
                reserved_regions,
            ));
        }

        // XXX: size including bootinfos?
        let size = std::mem::size_of::<LoaderHeader64>() as u64
            + region_metadata.iter().fold(0_u64, |acc, x| {
                acc + x.size + std::mem::size_of::<LoaderRegion64>() as u64
            });

        let header = LoaderHeader64 {
            magic,
            size,
            flags,
            num_multikernels: num_multikernels as u64,
            num_regions: region_metadata.len() as u64,
            kernel_v_entry: kernel_elf.entry,
        };

        Loader {
            image,
            header,
            kernel_bootinfos,
            region_metadata,
            regions: all_regions,
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

        for (bootinfo, kernel_regions, ram_regions, roottask_regions, reserved_regions) in
            self.kernel_bootinfos.iter()
        {
            let mut total_size = mem::size_of_val(bootinfo);

            loader_buf
                .write_all(unsafe { struct_to_bytes(bootinfo) })
                .expect("failed to write kernel bootinfo data to loader");

            // The ordering here needs to match what the kernel expects.

            for region in kernel_regions.iter() {
                total_size += mem::size_of_val(region);
                loader_buf
                    .write_all(unsafe { struct_to_bytes(region) })
                    .expect("failed to write kernel bootinfo data to loader");
            }
            for region in ram_regions.iter() {
                total_size += mem::size_of_val(region);
                loader_buf
                    .write_all(unsafe { struct_to_bytes(region) })
                    .expect("failed to write kernel bootinfo data to loader");
            }
            for region in roottask_regions.iter() {
                total_size += mem::size_of_val(region);
                loader_buf
                    .write_all(unsafe { struct_to_bytes(region) })
                    .expect("failed to write kernel bootinfo data to loader");
            }
            for region in reserved_regions.iter() {
                total_size += mem::size_of_val(region);
                loader_buf
                    .write_all(unsafe { struct_to_bytes(region) })
                    .expect("failed to write kernel bootinfo data to loader");
            }

            if total_size > 0x1000 {
                panic!("expected total size of bootinfo less than one page, got: {total_size:#x}");
            }

            // pack out to a page
            loader_buf
                .seek_relative(0x1000 - total_size as i64)
                .expect("couldn't seek");
        }

        // For each region, we need to write out the region metadata as well
        for region in &self.region_metadata {
            let region_metadata_bytes = unsafe { struct_to_bytes(region) };
            loader_buf
                .write_all(region_metadata_bytes)
                .expect("Failed to write region metadata to loader");
        }

        // Now we can write out all the region data
        for (_, data, _) in &self.regions {
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
    ) -> Vec<(u64, u64, [u8; PAGE_TABLE_SIZE])> {
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

        let mut boot_lvl1_pt: [u8; PAGE_TABLE_SIZE] = [0; PAGE_TABLE_SIZE];
        {
            let text_index_lvl1 = Riscv64::pt_index(num_pt_levels, text_addr, 1);
            let pt_entry = Riscv64::pte_next(boot_lvl2_pt_elf_addr);
            let start = 8 * text_index_lvl1;
            let end = start + 8;
            boot_lvl1_pt[start..end].copy_from_slice(&pt_entry.to_le_bytes());
        }

        let mut boot_lvl2_pt_elf: [u8; PAGE_TABLE_SIZE] = [0; PAGE_TABLE_SIZE];
        {
            let text_index_lvl2 = Riscv64::pt_index(num_pt_levels, text_addr, 2);
            for (page, i) in (text_index_lvl2..512).enumerate() {
                let start = 8 * i;
                let end = start + 8;
                let addr = text_addr + ((page as u64) << Riscv64::BLOCK_BITS_2MB);
                let pt_entry = Riscv64::pte_leaf(addr);
                boot_lvl2_pt_elf[start..end].copy_from_slice(&pt_entry.to_le_bytes());
            }
        }

        {
            let index = Riscv64::pt_index(num_pt_levels, first_vaddr, 1);
            let start = 8 * index;
            let end = start + 8;
            boot_lvl1_pt[start..end]
                .copy_from_slice(&Riscv64::pte_next(boot_lvl2_pt_addr).to_le_bytes());
        }

        let mut boot_lvl2_pt: [u8; PAGE_TABLE_SIZE] = [0; PAGE_TABLE_SIZE];

        {
            let index = Riscv64::pt_index(num_pt_levels, first_vaddr, 2);
            for (page, i) in (index..512).enumerate() {
                let start = 8 * i;
                let end = start + 8;
                let addr = first_paddr + ((page as u64) << Riscv64::BLOCK_BITS_2MB);
                let pt_entry = Riscv64::pte_leaf(addr);
                boot_lvl2_pt[start..end].copy_from_slice(&pt_entry.to_le_bytes());
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
        offset: u64,
    ) -> Vec<(u64, u64, [u8; PAGE_TABLE_SIZE])> {
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

        let boot_lvl1_lower_addr = boot_lvl1_lower_addr + offset;
        let boot_lvl1_upper_addr = boot_lvl1_upper_addr + offset;
        let boot_lvl2_upper_addr = boot_lvl2_upper_addr + offset;
        let boot_lvl0_lower_addr = boot_lvl0_lower_addr + offset;
        let boot_lvl0_upper_addr = boot_lvl0_upper_addr + offset;

        let mut boot_lvl0_lower: [u8; PAGE_TABLE_SIZE] = [0; PAGE_TABLE_SIZE];
        boot_lvl0_lower[..8].copy_from_slice(&(boot_lvl1_lower_addr | 3).to_le_bytes());

        let mut boot_lvl1_lower: [u8; PAGE_TABLE_SIZE] = [0; PAGE_TABLE_SIZE];
        for i in 0..512 {
            #[allow(clippy::identity_op)] // keep the (0 << 2) for clarity
            let pt_entry: u64 = ((i as u64) << AARCH64_1GB_BLOCK_BITS) |
                (1 << 10) | // access flag
                (0 << 2) | // strongly ordered memory
                (1); // 1G block
            let start = 8 * i;
            let end = 8 * (i + 1);
            boot_lvl1_lower[start..end].copy_from_slice(&pt_entry.to_le_bytes());
        }

        let boot_lvl0_upper: [u8; PAGE_TABLE_SIZE] = [0; PAGE_TABLE_SIZE];
        {
            let pt_entry = (boot_lvl1_upper_addr | 3).to_le_bytes();
            let idx = Aarch64::lvl0_index(first_vaddr);
            boot_lvl0_lower[8 * idx..8 * (idx + 1)].copy_from_slice(&pt_entry);
        }

        let mut boot_lvl1_upper: [u8; PAGE_TABLE_SIZE] = [0; PAGE_TABLE_SIZE];
        {
            let pt_entry = (boot_lvl2_upper_addr | 3).to_le_bytes();
            let idx = Aarch64::lvl1_index(first_vaddr);
            boot_lvl1_upper[8 * idx..8 * (idx + 1)].copy_from_slice(&pt_entry);
        }

        let mut boot_lvl2_upper: [u8; PAGE_TABLE_SIZE] = [0; PAGE_TABLE_SIZE];

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
            boot_lvl2_upper[start..end].copy_from_slice(&pt_entry.to_le_bytes());
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
