//
// Copyright 2024, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//
use crate::elf::{ElfFile, ElfSegmentData};
use crate::sel4::{Arch, Config, seL4_KernelBootInfo, seL4_KernelBoot_KernelRegion, seL4_KernelBoot_RamRegion,
    seL4_KernelBoot_RootTaskRegion, seL4_KernelBoot_ReservedRegion, SEL4_KERNEL_BOOT_INFO_MAGIC, SEL4_KERNEL_BOOT_INFO_VERSION_0};
use crate::uimage::uimage_serialise;
use crate::capdl::initialiser::CapDLInitialiser;
use crate::util::{mask, mb, round_up, struct_to_bytes};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::ops::Range;
use std::path::Path;
use std::iter::zip;
use std::mem;
use crate::MemoryRegion;

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
    let mut checked: Vec<(u64, u64, &String)> = Vec::new();
    for (base, data, name) in regions {
        let end = base + data.len() as u64;
        // Check that this does not overlap with any checked regions
        for (b, e, checked_name) in &checked {
            if !(end <= *b || *base >= *e) {
                panic!("Overlapping regions: {name}: [{base:x}..{end:x}) overlaps {checked_name}:[{b:x}..{e:x})");
            }
        }

        checked.push((*base, end, name));
    }
}

#[repr(C)]
struct LoaderRegion64 {
    load_addr: u64,
    load_size: u64,
    write_size: u64,
    offset: u64,
    r#type: u64,
}

#[repr(C)]
struct LoaderHeader64 {
    magic: u64,
    size: u64,
    flags: u64,
    num_multikernels: u64,
    // kernel_entry: u64,
    // ui_p_reg_start: u64,
    // ui_p_reg_end: u64,
    // pv_offset: u64,
    // v_entry: u64,
    num_regions: u64,
    kernel_v_entry: u64,
}

pub struct Loader<'a> {
    // arch: Arch,
    loader_image: Vec<u8>,
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
    // word_size: usize,
    // elf_machine: u16,
    // entry: u64,
}

impl<'a> Loader<'a> {
    pub fn new(
        config: &Config,
        loader_elf_path: &Path,
        kernel_elf: &'a ElfFile,
        kernel_elf_pv_offsets: &[u64],
        capdl_initialisers: &'a [CapDLInitialiser],
        // system_regions: Vec<(u64, &'a [u8])>,
        per_core_ram_regions: &[&[MemoryRegion]],
        shared_memory_phys_regions: &[MemoryRegion],
        // initial_task_elf: &'a ElfFile,
        // initial_task_phy_base: u64,
        // initial_task_vaddr_range: &Range<u64>,
    ) -> Loader<'a> {
        if config.arch == Arch::X86_64 {
            unreachable!("internal error: x86_64 does not support creating a loader image");
        }

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
        println!("Received number {}", num_multikernels);
        assert!(num_multikernels > 0);

        let mut kernel_regions = Vec::new();
        let mut inittask_regions: Vec<Vec<(u64, &'a [u8])>> = Vec::new();

        // Delete it.
        #[allow(unused_variables)]
        let kernel_elf_p_v_offset = ();

        let loadable_kernel_segments: Vec<_> = kernel_elf.loadable_segments();
        let kernel_first_vaddr = loadable_kernel_segments
            .first()
            .expect("kernel has at least one loadable segment")
            .virt_addr;

        let mut kernel_first_paddrs: Vec<u64> = Vec::new();
        for kernel_p_v_offset in kernel_elf_pv_offsets {
            let kernel_first_paddr = kernel_first_vaddr - kernel_p_v_offset;
            kernel_first_paddrs.push(kernel_first_paddr);

            for segment in &loadable_kernel_segments {
                let region_paddr = segment.virt_addr - kernel_p_v_offset;
                kernel_regions.push((region_paddr, segment.data().as_slice()));
            }
        }

        println!("RUST LOADER|FINISHED COLLECTING KERNEL REGIONS");
        let mut inittask_num_regions = 0;

        for multikernel_idx in 0..num_multikernels {
            // We support an initial task ELF with multiple segments. This is implemented by amalgamating all the segments
            // into 1 segment, so if your segments are sparse, a lot of memory will be wasted.
            let initial_task_segments = capdl_initialisers[multikernel_idx].elf.loadable_segments();

            let mut core_init_task_regions: Vec<(u64, &'a [u8])> = Vec::new();

            for segment in initial_task_segments.iter() {
                if segment.mem_size() > 0 {
                    let segment_paddr =
                        capdl_initialisers[multikernel_idx].phys_base.unwrap() + (segment.virt_addr - capdl_initialisers[multikernel_idx].image_bound().start);
                    core_init_task_regions.push((segment_paddr, segment.data()));
                }
            }
            inittask_regions.push(core_init_task_regions);
            inittask_num_regions += 1;
        }

        println!("RUST LOADER|FINISHED COLLECTING INITIAL TASK REGIONS");
        // Determine the pagetable variables
        // assert!(kernel_first_vaddr.is_some());
        // assert!(kernel_first_vaddr.is_some());

        let mut pagetable_vars: Vec<_> = Vec::new();

        for (idx, kernel_first_paddr) in kernel_first_paddrs.iter().enumerate() {
            let pagetables = match config.arch {
                Arch::Aarch64 => Loader::aarch64_setup_pagetables(
                    &loader_elf,
                    kernel_first_vaddr,
                    *kernel_first_paddr,
                    (idx * PAGE_TABLE_SIZE) as u64,
                ),
                Arch::Riscv64 => Loader::riscv64_setup_pagetables(
                    config,
                    &loader_elf,
                    kernel_first_vaddr,
                    *kernel_first_paddr,
                ),
                Arch::X86_64 => unreachable!("x86_64 does not support creating a loader image"),
            };
            pagetable_vars.push(pagetables);
        }

        println!("RUST LOADER|FINISHED CREATING PAGE TABLES");

        let image_segment = loader_elf
            .segments
            .into_iter()
            .find(|segment| segment.loadable)
            .expect("Did not find loadable segment");
        let image_vaddr = image_segment.virt_addr;
        // We have to clone here as the image executable is part of this function return object,
        // and the loader ELF is deserialised in this scope, so its lifetime will be shorter than
        // the return object.
        let mut loader_image = image_segment.data().clone();

        if image_vaddr != loader_elf.entry {
            panic!("The loader entry point must be the first byte in the image");
        }

        println!("RUST LOADER|COPYING PAGE TABLES INTO THE LOADER");
        for multikernel_idx in 0..num_multikernels {
            for (var_addr, var_size, var_data) in &pagetable_vars[multikernel_idx] {
                let offset = var_addr - image_vaddr;
                let var_size = var_size / (num_multikernels as u64);
                assert!(var_size == var_data.len() as u64);
                assert!(offset > 0);
                assert!(offset <= loader_image.len() as u64);
                loader_image[offset as usize..(offset + var_size) as usize].copy_from_slice(var_data);
            }
        }

        // Combine all the init task, kernel and system regions
        let mut all_regions: Vec<(u64, &[u8], String)> = Vec::with_capacity(
            inittask_num_regions + kernel_regions.len());

        // First, add the kernel regions
        for (kernel_idx, region) in kernel_regions.iter().enumerate() {
            all_regions.push((region.0, region.1, format!("kernel {kernel_idx}")));
        }

        // Add all the initial task regions. There can be multiple init task regions per multikernel
        for idx in 0..num_multikernels {
            let core_init_task_regions = &inittask_regions[idx];
            for (region_idx, region) in core_init_task_regions.iter().enumerate() {
                all_regions.push((region.0, region.1, format!("loader {idx} region {region_idx}")));
            }
        }

        // This clone isn't too bad as it is just a Vec<(u64, &[u8])>
        let mut all_regions_with_loader = all_regions.clone();
        all_regions_with_loader.push((image_vaddr, &loader_image, format!{"altloader"}));
        check_non_overlapping(&all_regions_with_loader);

        let mut region_metadata = Vec::new();
        let mut offset: u64 = 0;
        for (addr, data, _) in &all_regions {
            // @kwinter: Is it necessary to have load and write size here?
            region_metadata.push(LoaderRegion64 {
                load_addr: *addr,
                load_size: data.len() as u64,
                write_size: data.len() as u64,
                offset,
                r#type: 1,
            });
            offset += data.len() as u64;
        }

        // Add all the shared memory regions to the region metadata vector
        // @kwinter: Do we have to handle this differently when we handle
        // "filled" frames? We should be adding the binary data here
        for shared_mr in shared_memory_phys_regions.iter() {
            region_metadata.push(LoaderRegion64 {
                load_addr: shared_mr.base,
                load_size: 0,
                write_size: shared_mr.size(),
                offset: 0,
                r#type: 1,
            })
        }

        // @kwinter: The following code hasn't been vetted. And address TODO's
        let mut kernel_bootinfos = Vec::new();
        for (&raw_ram_regions, (kernel_first_paddr, core_init_task)) in zip(
            per_core_ram_regions, zip(kernel_first_paddrs, capdl_initialisers)) {
            let kernel_regions = vec![seL4_KernelBoot_KernelRegion {
                base: kernel_first_paddr,
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

            let init_task_phy_base = core_init_task.phys_base.unwrap();

            let init_task_size = core_init_task.image_bound().end - core_init_task.image_bound().start;

            let root_task_regions = vec![
                // TODO: remove pv_offset
                seL4_KernelBoot_RootTaskRegion {
                    paddr_base: init_task_phy_base,
                    paddr_end: init_task_phy_base + init_task_size,
                    vaddr_base: core_init_task.image_bound().start,
                    _padding: [0; 8],
                },
            ];

            // @kwinter: I'm fairly sure we dont need this anymore
            // let mut reserved_regions = vec![seL4_KernelBoot_ReservedRegion {
            //     base: extra_device_region.base,
            //     end: extra_device_region.end,
            // }];

            let mut reserved_regions = vec![];

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

            let info = seL4_KernelBootInfo {
                magic: SEL4_KERNEL_BOOT_INFO_MAGIC,
                version: SEL4_KERNEL_BOOT_INFO_VERSION_0,
                _padding0: [0; 3],
                root_task_entry: core_init_task.elf.entry,
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
                num_mpidrs: num_multikernels
                    .try_into()
                    .expect("cannot fit # mpidrs into u8"),
                _padding: [0; 3],
            };

            kernel_bootinfos.push((
                info,
                kernel_regions,
                ram_regions,
                root_task_regions,
                reserved_regions,
            ));
        }

        let size = std::mem::size_of::<LoaderHeader64>() as u64
            + region_metadata.iter().fold(0_u64, |acc, x| {
                acc + x.load_size + std::mem::size_of::<LoaderRegion64>() as u64
            })
            // @kwinter: Is this assuming a small page per kernel boot info?
            // Don't use magic numbers here
            + kernel_bootinfos.len() as u64 * 0x1000;

        // @kwinter: Do we need flags anymore?
        let flags = match config.hypervisor {
            true => 1,
            false => 0,
        };

        let header = LoaderHeader64 {
            magic,
            size,
            flags,
            num_multikernels: num_multikernels as u64,
            // kernel_entry,
            // ui_p_reg_start,
            // ui_p_reg_end,
            // pv_offset,
            // v_entry: inittask_v_entry,
            num_regions: region_metadata.len() as u64,
            kernel_v_entry: kernel_elf.entry,
        };

        // Loader {
        //     arch: config.arch,
        //     loader_image,
        //     header,
        //     region_metadata,
        //     regions,
        //     word_size: kernel_elf.word_size,
        //     elf_machine: kernel_elf.machine,
        //     entry: loader_elf.entry,
        // }

        Loader {
            loader_image,
            header,
            kernel_bootinfos,
            region_metadata,
            regions: all_regions,
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // First copy image data, which includes the Microkit bootloader's code, etc
        bytes.extend_from_slice(&self.loader_image);
        // Then we copy the loader metadata (known as the 'header')
        bytes.extend_from_slice(unsafe { struct_to_bytes(&self.header) });

        for (bootinfo, kernel_regions, ram_regions, roottask_regions, reserved_regions) in
            self.kernel_bootinfos.iter()
        {
            let mut total_size = mem::size_of_val(bootinfo);

            bytes.extend_from_slice(unsafe { struct_to_bytes(bootinfo) });

            // The ordering here needs to match what the kernel expects.

            for region in kernel_regions.iter() {
                total_size += mem::size_of_val(region);
                bytes.extend_from_slice(unsafe { struct_to_bytes(region) });
            }
            for region in ram_regions.iter() {
                total_size += mem::size_of_val(region);
                bytes.extend_from_slice(unsafe { struct_to_bytes(region) });
            }
            for region in roottask_regions.iter() {
                total_size += mem::size_of_val(region);
                bytes.extend_from_slice(unsafe { struct_to_bytes(region) });
            }
            for region in reserved_regions.iter() {
                total_size += mem::size_of_val(region);
                bytes.extend_from_slice(unsafe { struct_to_bytes(region) });
            }

            if total_size > 0x1000 {
                panic!("expected total size of bootinfo less than one page, got: {total_size:#x}");
            }

            // pack out to a page
            for _ in 0..0x1000 - total_size {
                bytes.push(0);
            }
        }

        // For each region, we need to copy the region metadata as well
        for region in &self.region_metadata {
            let region_metadata_bytes = unsafe { struct_to_bytes(region) };
            bytes.extend_from_slice(region_metadata_bytes);
        }
        // Now we can copy all the region data
        for (_, data, _) in &self.regions {
            bytes.extend_from_slice(data);
        }

        bytes
    }

    pub fn write_image(&self, path: &Path) {
        let loader_file = match File::create(path) {
            Ok(file) => file,
            Err(e) => panic!("Could not create '{}': {}", path.display(), e),
        };

        let mut loader_buf = BufWriter::new(loader_file);

        // First write out all the image data
        loader_buf
            .write_all(&self.to_bytes())
            .expect("Failed to write image data to loader");

        loader_buf.flush().unwrap();
    }

    // fn convert_to_elf(&self, path: &Path) -> ElfFile {
    //     let mut loader_elf = ElfFile::new(
    //         path.to_path_buf(),
    //         self.word_size,
    //         self.entry,
    //         self.elf_machine,
    //     );

    //     loader_elf.add_segment(
    //         true,
    //         true,
    //         true,
    //         self.entry,
    //         ElfSegmentData::RealData(self.to_bytes()),
    //     );

    //     loader_elf
    // }

    pub fn write_elf(&self, path: &Path) {
        // let loader_elf = self.convert_to_elf(path);

        // match loader_elf.reserialise(path) {
        //     Ok(_) => {}
        //     Err(e) => panic!("Could not create '{}': {}", path.display(), e),
        // }
        panic!("We are only building a binary image for now!\n");
    }

    pub fn write_uimage(&self, path: &Path) {
        // let executable_payload = self.to_bytes();
        // let entry_32: u32 = match <u64 as TryInto<u32>>::try_into(self.entry) {
        //     Ok(entry_32) => entry_32,
        //     Err(_) => panic!(
        //         "Could not create '{}': Loader link address 0x{:x} cannot be above 4G for uImage.",
        //         path.display(),
        //         self.entry
        //     ),
        // };

        // match uimage_serialise(&self.arch, entry_32, executable_payload, path) {
        //     Ok(_) => {}
        //     Err(e) => panic!("Could not create '{}': {}", path.display(), e),
        // }
        panic!("We are only building a binary image for now!\n");
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
