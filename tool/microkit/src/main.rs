//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

// we want our asserts, even if the compiler figures out they hold true already during compile-time
#![allow(clippy::assertions_on_constants)]

use microkit_tool::argparse;
use microkit_tool::argparse::{Args, ArgsError, RequestedImageType};
use microkit_tool::capdl::allocation::{
    simulate_capdl_object_alloc_algorithm, CapDLAllocEmulationErrorLevel,
};
use microkit_tool::capdl::build_capdl_spec;
use microkit_tool::capdl::initialiser::CapDLInitialiser;
use microkit_tool::capdl::packaging::pack_spec_into_initial_task;
use microkit_tool::elf::ElfFile;
use microkit_tool::loader::Loader;
use microkit_tool::report::write_report;
use microkit_tool::sdf::{parse, Map, SysMap, SysMemoryRegion, SysMemoryRegionPaddr};
use microkit_tool::sdk::{AvailableConfig, Sdk};
use microkit_tool::sel4::{
    emulate_kernel_boot, emulate_kernel_boot_partial, AddressSpaceConstants, Arch, Config,
    ObjectSizes, PageSize, PlatformConfig, RiscvVirtualMemory,
};
use microkit_tool::symbols::patch_symbols;
use microkit_tool::util::{
    get_full_path, human_size_strict, json_str, json_str_as_bool, json_str_as_u64, round_down,
    round_up,
};
use microkit_tool::viper;
use microkit_tool::{DisjointMemoryRegion, MemoryRegion};
use std::collections::HashMap;
use std::fs::{self, metadata};
use std::ops::Range;
use std::path::Path;

const MAX_BUILD_ITERATION: usize = 3;

// When building for x86, the kernel is copied from the SDK release package to the same
// directory as the output boot module image, as Multiboot want them as
// separate images.
const KERNEL_COPY_FILENAME: &str = "sel4.elf";
// The `-kernel` argument of 'qemu-system-x86_64' doesn't accept a 64-bit image, so we
// also copy the 32-bit version that was prepared by build_sdk.py for convenience.
const KERNEL32_COPY_FILENAME: &str = "sel4_32.elf";

enum ImageOutputType {
    Binary,
    Elf,
    Uimage,
}

impl ImageOutputType {
    fn default_from_arch_and_board(arch: &Arch, board_name: &str) -> Self {
        match board_name {
            "ariane" | "cheshire" | "serengeti" => ImageOutputType::Elf,
            _ => match arch {
                Arch::Aarch64 => ImageOutputType::Binary,
                Arch::Riscv64 => ImageOutputType::Uimage,
                Arch::X86_64 => ImageOutputType::Elf,
            },
        }
    }

    /// Resolve the optional user-specified image type with what is the default for the
    /// platform.
    /// Not all image types are supported for all platforms, so we check here.
    fn resolve(requested: &RequestedImageType, arch: &Arch, board_name: &str) -> Option<Self> {
        match requested {
            RequestedImageType::Binary => match arch {
                Arch::Aarch64 | Arch::Riscv64 => Some(Self::Binary),
                Arch::X86_64 => None,
            },
            RequestedImageType::Elf => Some(Self::Elf),
            RequestedImageType::Uimage => match arch {
                Arch::Riscv64 => Some(Self::Uimage),
                Arch::X86_64 | Arch::Aarch64 => None,
            },
            RequestedImageType::Unspecified => {
                Some(Self::default_from_arch_and_board(arch, board_name))
            }
        }
    }
}

fn bail_if_not_exists(description: &'static str, path: &Path) -> Result<(), String> {
    if !path.exists() {
        eprintln!(
            "microkit: error: {description} '{}' does not exist",
            path.display()
        );
        std::process::exit(1);
    }
    Ok(())
}

fn parse_json_file<T: serde::de::DeserializeOwned>(
    file_name: &str,
    file_description: &'static str,
    current_config: &AvailableConfig,
) -> Result<T, String> {
    let path = current_config.config_dir.join(file_name);
    bail_if_not_exists(file_description, &path)?;
    serde_json::from_str(&fs::read_to_string(&path).expect("Error: Unable to read {path}"))
        .map_err(|err| format!("Error: Unable to parse {file_name}: {err}"))
}

fn main() -> Result<(), String> {
    let sdk = match Sdk::discover() {
        Ok(discovered_info) => discovered_info,
        Err(err) => {
            eprintln!("microkit: error: {err}");
            std::process::exit(1);
        }
    };

    let env_args: Vec<_> = std::env::args().collect();
    let mut args = match Args::parse(&env_args, &sdk) {
        Ok(parsed_arguments) => parsed_arguments,
        Err(ArgsError::HelpWanted) => {
            argparse::print_help(&sdk);
            std::process::exit(0);
        }
        Err(err) => {
            match err {
                ArgsError::UnrecognizedArgument { arg: _ }
                | ArgsError::MissingRequiredArguments { args: _ } => {
                    argparse::print_usage();
                }
                _ => {}
            };
            eprintln!("microkit: error: {err}");
            std::process::exit(1);
        }
    };
    args.search_paths.push(sdk.cwd.clone());

    // NB safe unwrap: argparse would already have bailed if the config did not
    // exist.
    let current_config = sdk.select(&args.board, &args.config).unwrap();

    // the real work begins here
    let elf_path = current_config.config_dir.join("elf");
    let loader_elf_path = elf_path.join("loader.elf");
    let kernel_elf_path = match args.override_kernel {
        Some(ref path) => path,
        None => &elf_path.join("sel4.elf"),
    };
    let monitor_elf_path = elf_path.join("monitor.elf");
    let capdl_init_elf_path = elf_path.join("initialiser.elf");
    let kernel_config_path = current_config
        .config_dir
        .join("include/kernel/gen_config.json");
    let invocations_all_path = current_config.config_dir.join("invocations_all.json");
    bail_if_not_exists("board ELF directory", &elf_path)?;
    bail_if_not_exists("kernel ELF", kernel_elf_path)?;
    bail_if_not_exists("monitor ELF", &monitor_elf_path)?;
    bail_if_not_exists("CapDL initialiser ELF", &capdl_init_elf_path)?;
    bail_if_not_exists("kernel configuration file", &kernel_config_path)?;
    bail_if_not_exists("invocations JSON file", &invocations_all_path)?;

    let system_path = &args.sdf_path;
    bail_if_not_exists("system description file", system_path)?;

    let xml: String = fs::read_to_string(system_path).unwrap();

    let kernel_config_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(kernel_config_path).unwrap()).unwrap();

    let invocations_labels: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(invocations_all_path).unwrap()).unwrap();

    let arch = match json_str(&kernel_config_json, "SEL4_ARCH")? {
        "aarch64" => Arch::Aarch64,
        "riscv64" => Arch::Riscv64,
        "x86_64" => Arch::X86_64,
        _ => panic!("Unsupported kernel config architecture"),
    };

    let image_output_type = match ImageOutputType::resolve(
        &args.requested_image_type,
        &arch,
        args.board.as_str(),
    ) {
        Some(image) => image,
        None => {
            eprintln!(
                    "microkit: error: building the output image as '{0}' is unsupported for target architecture '{arch}'",
                    args.requested_image_type
                );
            std::process::exit(1);
        }
    };

    let (device_regions, normal_regions) = match arch {
        Arch::X86_64 => (None, None),
        _ => {
            let platform_gen_path = current_config.config_dir.join("platform_gen.json");
            bail_if_not_exists("kernel platform configuration file", &platform_gen_path)?;
            let kernel_platform_config: PlatformConfig =
                serde_json::from_str(&fs::read_to_string(platform_gen_path).unwrap()).unwrap();

            (
                Some(kernel_platform_config.devices),
                Some(kernel_platform_config.memory),
            )
        }
    };

    let object_sizes: ObjectSizes = parse_json_file(
        "object_sizes.json",
        "kernel object sizes file",
        current_config,
    )?;

    let address_space_constants: AddressSpaceConstants = parse_json_file(
        "address_space_constants.json",
        "kernel address space constants file",
        current_config,
    )?;

    let hypervisor = match arch {
        Arch::Aarch64 => json_str_as_bool(&kernel_config_json, "ARM_HYPERVISOR_SUPPORT")?,
        Arch::X86_64 => json_str_as_bool(&kernel_config_json, "VTX")?,
        // Hypervisor mode is not available on RISC-V
        _ => false,
    };

    let iommu = match arch {
        Arch::X86_64 => json_str_as_bool(&kernel_config_json, "IOMMU")?,
        _ => false,
    };

    let arm_pa_size_bits = match arch {
        Arch::Aarch64 => {
            if json_str_as_bool(&kernel_config_json, "ARM_PA_SIZE_BITS_40")? {
                Some(40)
            } else if json_str_as_bool(&kernel_config_json, "ARM_PA_SIZE_BITS_44")? {
                Some(44)
            } else {
                panic!("Expected ARM platform to have 40 or 44 physical address bits")
            }
        }
        Arch::X86_64 | Arch::Riscv64 => None,
    };

    let arm_smc = match arch {
        Arch::Aarch64 => Some(json_str_as_bool(&kernel_config_json, "ALLOW_SMC_CALLS")?),
        _ => None,
    };

    let kernel_frame_size = match arch {
        Arch::Aarch64 => 1 << 12,
        Arch::Riscv64 => 1 << 21,
        Arch::X86_64 => 1 << 12,
    };

    let kernel_config = Config {
        arch,
        word_size: json_str_as_u64(&kernel_config_json, "WORD_SIZE")?,
        minimum_page_size: 1 << object_sizes.small_page,
        paddr_user_device_top: json_str_as_u64(&kernel_config_json, "PADDR_USER_DEVICE_TOP")?,
        kernel_frame_size,
        init_cnode_bits: json_str_as_u64(&kernel_config_json, "ROOT_CNODE_SIZE_BITS")?,
        cap_address_bits: 64,
        fan_out_limit: json_str_as_u64(&kernel_config_json, "RETYPE_FAN_OUT_LIMIT")?,
        max_num_bootinfo_untypeds: json_str_as_u64(
            &kernel_config_json,
            "MAX_NUM_BOOTINFO_UNTYPED_CAPS",
        )?,
        hypervisor,
        iommu,
        benchmark: args.config == "benchmark",
        num_cores: if json_str_as_bool(&kernel_config_json, "ENABLE_SMP_SUPPORT")? {
            json_str_as_u64(&kernel_config_json, "MAX_NUM_NODES")?
                .try_into()
                .expect("number of cores fits in u8")
        } else {
            1
        },
        num_domains: json_str_as_u64(&kernel_config_json, "NUM_DOMAINS")?
            .try_into()
            .unwrap(),
        num_domain_schedules: json_str_as_u64(&kernel_config_json, "NUM_DOMAIN_SCHEDULES")?,
        fpu: json_str_as_bool(&kernel_config_json, "HAVE_FPU")?,
        arm_pa_size_bits,
        arm_smc,
        riscv_pt_levels: Some(RiscvVirtualMemory::Sv39),
        invocations_labels,
        device_regions,
        normal_regions,
        object_sizes,
        address_space_constants,
    };

    if kernel_config.arch != Arch::X86_64 && !loader_elf_path.exists() {
        eprintln!(
            "Error: loader ELF '{}' does not exist",
            loader_elf_path.display()
        );
        std::process::exit(1);
    }

    assert!(
        kernel_config.word_size == 64,
        "Microkit tool has various assumptions about the word size being 64-bits."
    );

    let mut system = match parse(
        system_path.as_path(),
        &xml,
        &kernel_config,
        &args.search_paths,
    ) {
        Ok(system) => system,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    let capdl_initialiser_elf = ElfFile::from_path(&capdl_init_elf_path).unwrap_or_else(|e| {
        eprintln!(
            "ERROR: failed to parse initialiser ELF ({}): {}",
            capdl_init_elf_path.display(),
            e
        );
        std::process::exit(1);
    });

    // Only relevant for ARM and RISC-V.
    // Determine how much physical memory is available to the kernel after it boots but before dropping
    // to userspace by partially emulating the kernel boot process. This is useful for two purposes:
    // 1. To implement setvar region_paddr for memory regions that doesn't specify a phys address, where
    //    we must automatically select a suitable address inside the Microkit tool.
    // 2. Post-spec generation sanity checks at a later point to ensure that there are sufficient memory
    //    to allocate all kernel objects.
    let (kernel_elf_maybe, available_memory_maybe, kernel_boot_region_maybe) =
        match kernel_config.arch {
            Arch::X86_64 => (None, None, None),
            Arch::Aarch64 | Arch::Riscv64 => {
                let kernel_elf = ElfFile::from_path(kernel_elf_path).unwrap_or_else(|e| {
                    eprintln!(
                        "ERROR: failed to parse kernel ELF ({}): {}",
                        kernel_elf_path.display(),
                        e
                    );
                    std::process::exit(1);
                });

                // Now determine how much memory we have after the kernel boots.
                let (available_memory, kernel_boot_region) =
                    emulate_kernel_boot_partial(&kernel_config, &kernel_elf);
                (
                    Some(kernel_elf),
                    Some(available_memory),
                    Some(kernel_boot_region),
                )
            }
        };

    let monitor_elf = ElfFile::from_path(&monitor_elf_path).unwrap_or_else(|e| {
        eprintln!(
            "ERROR: failed to parse monitor ELF ({}): {}",
            monitor_elf_path.display(),
            e
        );
        std::process::exit(1);
    });

    // This list refers to all PD ELFs as well as the Monitor ELF.
    // The monitor is very similar to a PD so it is useful to pass around
    // a list like this.
    let mut system_elfs = Vec::with_capacity(system.protection_domains.len());
    // Get the elf files for each pd:
    for pd in &system.protection_domains {
        match get_full_path(&pd.program_image, &args.search_paths) {
            Some(path) => {
                let path_for_symbols = pd
                    .program_image_for_symbols
                    .as_ref()
                    .map(|path_suffix| {
                        get_full_path(path_suffix, &args.search_paths).ok_or_else(|| {
                            format!(
                                "unable to find program image for symbols: '{}'",
                                path_suffix.display()
                            )
                        })
                    })
                    .transpose()?;
                match ElfFile::from_split_paths(&path, path_for_symbols.as_deref()) {
                    Ok(elf) => system_elfs.push(elf),
                    Err(e) => {
                        eprintln!(
                            "ERROR: failed to parse ELF '{}' for PD '{}': {}",
                            path.display(),
                            pd.name,
                            e
                        );
                        std::process::exit(1);
                    }
                };
            }
            None => {
                return Err(format!(
                    "unable to find program image: '{}'",
                    pd.program_image.display()
                ))
            }
        }
    }

    // The monitor is just a special PD
    system_elfs.push(monitor_elf);

    let get_mr_size = |mr_name: &str| {
        system
            .memory_regions
            .iter()
            .find(|mr| &mr.name == mr_name)
            .expect("validated in sdf.rs")
            .size
    };
    let find_free_region = |regions: &mut [Range<u64>], size: u64, max_addr: u64| {
        let page_size = PageSize::Small as u64;
        let candidate_before = |possible_end| {
            let end = round_down(possible_end, page_size);
            let start = round_down(
                end.checked_sub(size)
                    .expect("Error: no free region in address space"),
                page_size,
            );
            start..end
        };

        regions.sort_by_key(|range| range.start);
        let mut possible_end = round_down(max_addr, page_size);
        for region in regions.iter().rev() {
            if possible_end <= region.start {
                continue;
            }
            let candidate = candidate_before(possible_end);
            if candidate.start >= region.end {
                return candidate;
            }
            possible_end = round_down(region.start, page_size);
        }
        candidate_before(possible_end)
    };
    let encode_metadata = |vaddr: u64, nested: bool| -> u64 {
        const PRESENT_BITS: u64 = 1;
        const NESTED_BITS: u64 = 1;
        const PAYLOAD_BITS: u64 = u64::BITS as u64 - PRESENT_BITS - NESTED_BITS;
        const PRESENT_BIT: u64 = 1 << (PAYLOAD_BITS + 1);
        const NESTED_BIT: u64 = 1 << PAYLOAD_BITS;
        if vaddr >= (1 << PAYLOAD_BITS) {
            panic!("Error: metadata vaddr {vaddr:#x} does not fit in {PAYLOAD_BITS} payload bits");
        }
        PRESENT_BIT | (if nested { NESTED_BIT } else { 0 }) | vaddr
    };

    let pd_elf_segments_by_idx: Vec<Vec<Range<u64>>> = system_elfs
        .iter()
        .map(|seg| {
            seg.loadable_segments()
                .iter()
                .map(|seg| {
                    let start = round_down(seg.virt_addr, PageSize::Small as u64);
                    let end = round_up(seg.virt_addr + seg.mem_size(), PageSize::Small as u64);
                    start..end
                })
                .collect()
        })
        .collect();

    let mut pd_regions: Vec<Vec<Range<u64>>> = system
        .protection_domains
        .iter()
        .map(|pd| {
            pd.maps
                .iter()
                .map(|map| map.vaddr..map.vaddr + get_mr_size(map.mr_name()))
                .chain(pd.page_tables.iter().map(|pt| pt.vaddr..pt.vaddr + pt.size))
                .collect::<Vec<Range<u64>>>()
        })
        .collect();

    for (idx, regions) in pd_regions.iter_mut().enumerate() {
        regions.extend_from_slice(&pd_elf_segments_by_idx[idx]);
        regions.sort_by_key(|range| range.start);
    }

    let pd_stack_bases_by_idx = system
        .protection_domains
        .iter()
        .map(|pd| kernel_config.pd_stack_bottom(pd.stack_size))
        .collect::<Vec<u64>>();

    for (pd_idx, pd) in system.protection_domains.iter_mut().enumerate() {
        let Some(cspace) = &mut pd.cspace else {
            continue;
        };

        // create the MR with the bytes for the metadata
        let mut root_metadata = vec![0u64; 1usize << cspace.size_bits];
        for cap_map in cspace.cap_maps.iter() {
            match cap_map {
                microkit_tool::sdf::CapMap::ElfFrames(map) => {
                    let other_pd_idx = map.pd_info().pd.expect("Filled in sdf.rs");
                    let frame_vaddrs = system_elfs[other_pd_idx]
                        .loadable_segments()
                        .iter()
                        .flat_map(|seg| {
                            let start = round_down(seg.virt_addr, PageSize::Small as u64);
                            let end =
                                round_up(seg.virt_addr + seg.mem_size(), PageSize::Small as u64);
                            (start..end).step_by(PageSize::Small as usize)
                        })
                        .collect::<Vec<u64>>();

                    // The last entry is a zero terminator.
                    let region_size_bytes = round_up(
                        ((frame_vaddrs.len() + 1) * 8) as u64,
                        PageSize::Small as u64,
                    );

                    let nested_cspace_region = find_free_region(
                        &mut pd_regions[pd_idx],
                        region_size_bytes,
                        kernel_config.pd_map_max_vaddr(pd.stack_size),
                    );
                    pd_regions[pd_idx].push(nested_cspace_region.clone());

                    root_metadata[cap_map.slot() as usize] =
                        encode_metadata(nested_cspace_region.start, true);

                    let nested_metadata = frame_vaddrs
                        .into_iter()
                        .flat_map(|vaddr| encode_metadata(vaddr, false).to_le_bytes())
                        .chain(0u64.to_le_bytes())
                        .collect::<Vec<u8>>();

                    let nested_mr_name =
                        format!("pd_{}_slot_{}_nested_metadata", pd.name, cap_map.slot());
                    let nested_mr =
                        SysMemoryRegion::new_mr(nested_mr_name.clone(), nested_metadata);
                    system.memory_regions.push(nested_mr);
                    pd.maps
                        .push(SysMap::new_map(nested_mr_name, nested_cspace_region.start));
                }
                microkit_tool::sdf::CapMap::StackFrames(map) => {
                    let other_pd_idx = map.pd_info().pd.expect("Filled in sdf.rs");
                    root_metadata[cap_map.slot() as usize] =
                        encode_metadata(pd_stack_bases_by_idx[other_pd_idx], false);
                }
                microkit_tool::sdf::CapMap::IpcBufferFrame(_) => {
                    root_metadata[cap_map.slot() as usize] =
                        encode_metadata(kernel_config.pd_ipc_buffer(), false);
                }
                _ => (),
            }
        }

        let root_cspace_metadata_region = find_free_region(
            &mut pd_regions[pd_idx],
            round_up((1u64 << cspace.size_bits) * 8, PageSize::Small as u64),
            kernel_config.pd_map_max_vaddr(pd.stack_size),
        );

        pd_regions[pd_idx].push(root_cspace_metadata_region.clone());
        let root_metadata_bytes = root_metadata
            .iter()
            .flat_map(|vaddr| vaddr.to_le_bytes())
            .collect::<Vec<u8>>();

        cspace.metadata_vaddr = Some(root_cspace_metadata_region.start);
        let root_mr_name = format!("pd_{}_slot_root_cspace_metadata", pd.name,);
        let root_mr = SysMemoryRegion::new_mr(root_mr_name.clone(), root_metadata_bytes);
        system.memory_regions.push(root_mr);
        pd.maps.push(SysMap::new_map(
            root_mr_name,
            root_cspace_metadata_region.start,
        ));
    }

    let capdl_initialiser_orig = CapDLInitialiser::new(capdl_initialiser_elf);

    // Now build the capDL spec and final image. We may need to do this in >1 iterations on ARM and RISC-V
    // if there are Memory Regions without a paddr but subject to setvar region_paddr.
    let mut iteration = 0;
    let mut spec_need_refinement = true;
    let mut system_built = false;
    while spec_need_refinement && iteration < MAX_BUILD_ITERATION {
        let mut capdl_initialiser = capdl_initialiser_orig.clone();
        spec_need_refinement = false;

        // Patch all the required symbols in the Monitor and PDs according to the Microkit's requirements
        if let Err(err) = patch_symbols(&kernel_config, &mut system_elfs, &system) {
            eprintln!("ERROR: {err}");
            std::process::exit(1);
        }

        let mut spec_container = build_capdl_spec(&kernel_config, &mut system_elfs, &system)?;
        pack_spec_into_initial_task(
            &kernel_config,
            args.config.as_str(),
            &spec_container,
            &system_elfs,
            &mut capdl_initialiser,
        );

        match kernel_config.arch {
            Arch::X86_64 => {
                // setvar region_paddr not supported on this architecture nor can we emulate the
                // kernel boot process to statically check for issues due to unknown memory map, so nothing to do.
                // Write out the capDL initialiser as an ELF boot module and we are done.
            }
            Arch::Aarch64 | Arch::Riscv64 => {
                // Now that we have the CapDL initialiser ELF with embedded spec,
                // we can determine exactly how much memory will be available statically when the kernel
                // drops to userspace on ARM and RISC-V. This allow us to sanity check that:
                // 1. There are enough memory to allocate all the objects required in the spec.
                // 2. All frames with a physical attached reside in legal memory (device or normal).
                // 3. Objects can be allocated from the free untyped list. For example, we detect
                //    situations where you might have a few frames with size bit 12 to allocate but
                //    only have untyped with size bit <12 remaining.
                // This also allow the tool to automatically pick physical address of Memory Regions with out
                // an explicit paddr in SDF but are subject to setvar region_paddr.

                // Determine how much memory the CapDL initialiser needs.
                let initialiser_vaddr_range = capdl_initialiser.image_bound();
                let initial_task_size = initialiser_vaddr_range.end - initialiser_vaddr_range.start;

                // Reuse data from the partial kernel boot emulation previously done.
                // .clone() as we need to mutate this for every iteration.
                let mut available_memory = available_memory_maybe.clone().unwrap();
                let kernel_boot_region = kernel_boot_region_maybe.unwrap();

                // The kernel relies on the initial task region being allocated above the kernel
                // boot/ELF region, so we have the end of the kernel boot region as the lower
                // bound for allocating the reserved region.
                let initial_task_phys_base =
                    available_memory.allocate_from(initial_task_size, kernel_boot_region.end);

                let Some(initial_task_phys_base) = initial_task_phys_base else {
                    // Unlikely to happen on Microkit-supported platforms with multi gigabytes memory.
                    // But printing a helpful error in case we do run into this problem.
                    eprintln!(
                        "ERROR: cannot allocate memory for the initialiser, contiguous physical memory region of size {} not found", human_size_strict(initial_task_size)
                    );
                    eprintln!("ERROR: physical memory regions the initialiser can be placed at:");
                    for region in available_memory.regions {
                        eprintln!(
                            "       [0x{:0>12x}..0x{:0>12x}), size: {}",
                            region.base,
                            region.end,
                            human_size_strict(region.size())
                        );
                    }
                    std::process::exit(1);
                };

                capdl_initialiser.set_phys_base(initial_task_phys_base);
                let initial_task_phys_region = MemoryRegion::new(
                    initial_task_phys_base,
                    initial_task_phys_base + initial_task_size,
                );
                let user_image_virt_region = MemoryRegion::new(
                    capdl_initialiser.elf.lowest_vaddr(),
                    initialiser_vaddr_range.end,
                );

                // With the initial task region determined the kernel boot can be emulated in full. This provides
                // the boot info information (containing untyped objects) which is needed for the next steps
                let kernel_boot_info = emulate_kernel_boot(
                    &kernel_config,
                    kernel_elf_maybe.as_ref().unwrap(),
                    initial_task_phys_region,
                    user_image_virt_region,
                );

                if iteration == 0 {
                    // On the first iteration where the spec have not been refined, simulate the capDL allocation algorithm
                    // to double check that all kernel objects of the system as described by SDF can be successfully allocated.
                    if !simulate_capdl_object_alloc_algorithm(
                        &mut spec_container,
                        &kernel_boot_info,
                        &kernel_config,
                        CapDLAllocEmulationErrorLevel::PrintStderr,
                    ) {
                        eprintln!("ERROR: could not allocate all required kernel objects. Please see report for more details.");
                        std::process::exit(1);
                    }
                } else {
                    // Do the same thing for further iterations, at this point the simulation won't fail *except* for when we have picked a
                    // bad address for Memory Regions subject to setvar region_paddr. This can happen because after we have
                    // picked the address, we will update spec and patch it into the program's frame. Which will causes the
                    // spec to increase in size as the frames' data are compressed. So if the simulation fail, we need to
                    // pick another address as we now have a better idea of how large the spec is.

                    // This is highly unlikely to happen unless the spec size increase causes the initial task size to cross
                    // a 4K page boundary.
                    if !simulate_capdl_object_alloc_algorithm(
                        &mut spec_container,
                        &kernel_boot_info,
                        &kernel_config,
                        CapDLAllocEmulationErrorLevel::Suppressed,
                    ) {
                        // Encountered a problem, pick a better address.
                        for tool_allocate_mr in system.memory_regions.iter_mut().filter(|mr| {
                            matches!(mr.phys_addr, SysMemoryRegionPaddr::ToolAllocated(_))
                        }) {
                            tool_allocate_mr.phys_addr = SysMemoryRegionPaddr::ToolAllocated(None);
                        }
                        spec_container.expected_allocations = HashMap::new();
                    }
                }

                // Now pick a physical address for any memory regions that are subject to setvar region_paddr.
                // Doing something a bit unconventional here: converting the list of untypeds back to a DisjointMemoryRegion
                // to give us a view of physical memory available after the kernel drops to user space.
                // I.e. available memory after the initial task have been created.
                {
                    let mut available_user_memory = DisjointMemoryRegion::default();
                    for ut in kernel_boot_info
                        .untyped_objects
                        .iter()
                        .filter(|ut| !ut.is_device)
                    {
                        // Only take untypeds that can at least fit a page because some have been used to back the initial task's
                        // kernel object such as TCB, endpoint etc.
                        let start = round_up(ut.base(), kernel_config.minimum_page_size);
                        let end = round_down(ut.end(), kernel_config.minimum_page_size);
                        if end > start {
                            // will be automatically merged
                            available_user_memory.insert_region(ut.base(), ut.end());
                        }
                    }

                    // Then take away any memory ranges occupied by Memory Regions with a paddr specified in SDF.
                    for mr in system.memory_regions.iter() {
                        if let SysMemoryRegionPaddr::Specified(sdf_paddr) = mr.phys_addr {
                            let mr_end = sdf_paddr + mr.size;

                            // MR may be device memory, which isn't covered in available_user_memory.
                            let is_normal_mem =
                                available_user_memory.regions.iter().any(|region| {
                                    sdf_paddr >= region.base
                                        && sdf_paddr < region.end
                                        && mr_end <= region.end
                                });
                            if is_normal_mem {
                                available_user_memory.remove_region(sdf_paddr, sdf_paddr + mr.size);
                            }
                        }
                    }

                    let mut tool_allocated_mrs = Vec::new();
                    for (mr_id, tool_allocate_mr) in system
                        .memory_regions
                        .iter_mut()
                        .enumerate()
                        .filter(|(_, mr)| {
                            matches!(mr.phys_addr, SysMemoryRegionPaddr::ToolAllocated(None))
                        })
                    {
                        spec_need_refinement = true;

                        let target_paddr = available_user_memory
                            .allocate(tool_allocate_mr.size, tool_allocate_mr.page_size);
                        if target_paddr.is_none() {
                            eprintln!("ERROR: cannot auto-select a physical address for MR {} because there are no contiguous memory region of sufficient size.", tool_allocate_mr.name);
                            eprintln!("ERROR: MR {} needs to be physically contiguous as it is a subject of a setvar region_paddr.", tool_allocate_mr.name);
                            if !tool_allocated_mrs.is_empty() {
                                eprintln!("Previously auto-allocated memory regions:");
                                for allocated_mr_id in tool_allocated_mrs {
                                    let allocated_mr: &SysMemoryRegion =
                                        &system.memory_regions[allocated_mr_id];
                                    eprintln!(
                                        "name = '{}', paddr = 0x{:0>12x}, size = 0x{:0>12x}",
                                        allocated_mr.name,
                                        allocated_mr.paddr().unwrap(),
                                        allocated_mr.size
                                    );
                                }
                            }
                            eprintln!("available physical memory regions:");
                            for region in available_user_memory.regions {
                                eprintln!(
                                    "[0x{:0>12x}..0x{:0>12x}), size: {}",
                                    region.base,
                                    region.end,
                                    human_size_strict(region.size())
                                );
                            }
                            std::process::exit(1);
                        }
                        tool_allocated_mrs.push(mr_id);
                        tool_allocate_mr.phys_addr =
                            SysMemoryRegionPaddr::ToolAllocated(target_paddr);
                    }
                }

                // Patch the list of untypeds we used to simulate object allocation into the initialiser.
                // At runtime the initialiser will validate what we simulated against what the kernel gives it. If they deviate
                // we will have problems! For example, if we simulated with more memory than what's actually available, the initialiser
                // can crash.
                capdl_initialiser.add_expected_untypeds(&kernel_boot_info.untyped_objects);
            }
        };

        if !spec_need_refinement {
            // All is well in the universe, write the image out.
            println!(
                "MICROKIT|CAPDL SPEC: number of root objects = {}, spec footprint = {}",
                spec_container.spec.objects.len(),
                human_size_strict(
                    capdl_initialiser
                        .spec_metadata()
                        .as_ref()
                        .unwrap()
                        .spec_size
                ),
            );
            let initialiser_vaddr_range = capdl_initialiser.image_bound();
            println!(
                "MICROKIT|INITIAL TASK: memory size = {}",
                human_size_strict(initialiser_vaddr_range.end - initialiser_vaddr_range.start),
            );

            let image_out_path = args.output_path.as_path();

            match kernel_config.arch {
                Arch::X86_64 => match capdl_initialiser.elf.reserialise(image_out_path) {
                    Ok(size) => {
                        // Copy the kernel to the build directory as well so users doesn't have to dig through the SDK.
                        if let Err(copy_err) = fs::copy(
                            kernel_elf_path,
                            image_out_path.parent().unwrap().join(KERNEL_COPY_FILENAME),
                        ) {
                            eprintln!("ERROR: couldn't copy the kernel to image's output directory: {copy_err}");
                            std::process::exit(1);
                        }
                        if let Err(copy_err) = fs::copy(
                            kernel_elf_path
                                .parent()
                                .unwrap()
                                .join(KERNEL32_COPY_FILENAME),
                            image_out_path
                                .parent()
                                .unwrap()
                                .join(KERNEL32_COPY_FILENAME),
                        ) {
                            eprintln!("ERROR: couldn't copy the 32-bit kernel to image's output directory: {copy_err}");
                            std::process::exit(1);
                        }
                        println!(
                            "MICROKIT|BOOT MODULE: image file size = {}",
                            human_size_strict(size)
                        );
                    }
                    Err(err) => {
                        eprintln!("ERROR: couldn't write the boot module to filesystem: {err}");
                        std::process::exit(1);
                    }
                },
                Arch::Aarch64 | Arch::Riscv64 => {
                    let loader = Loader::new(
                        &kernel_config,
                        Path::new(&loader_elf_path),
                        kernel_elf_maybe.as_ref().unwrap(),
                        &capdl_initialiser.elf,
                        capdl_initialiser.phys_base.unwrap(),
                        &initialiser_vaddr_range,
                    );

                    match image_output_type {
                        ImageOutputType::Binary => loader.write_image(image_out_path),
                        ImageOutputType::Elf => loader.write_elf(image_out_path),
                        ImageOutputType::Uimage => loader.write_uimage(image_out_path),
                    };

                    println!(
                        "MICROKIT|LOADER: image file size = {}",
                        human_size_strict(metadata(image_out_path).unwrap().len())
                    );
                }
            };

            if let Some(capdl_json) = args.capdl_json_path {
                let serialised = serde_json::to_string_pretty(&spec_container.spec).unwrap();
                fs::write(capdl_json, &serialised).unwrap();
            };

            if let Some(viper_output_dir) = args.viper_output_dir {
                // NB returns Ok if the directory already exists, that's fine
                fs::create_dir_all(&viper_output_dir).unwrap_or_else(|source| {
                    eprintln!(
                        "ERROR: cannot write Viper output directory {}: {source}",
                        &viper_output_dir.display()
                    );
                    std::process::exit(1);
                });
                for view in viper::get_combined_views(&spec_container, &system) {
                    let mut output = format!(
                        "// exported invariants for PD {} in {}\n",
                        view.pd_name,
                        &args.sdf_path.display(),
                    );
                    view.export(&mut output);
                    let path = viper_output_dir.join(format!("{}.vpr", view.pd_name));
                    fs::write(&path, output).unwrap_or_else(|source| {
                        eprintln!(
                            "ERROR: cannot write Viper output file {}: {source}",
                            &path.display()
                        );
                        std::process::exit(1);
                    });
                }
            }

            write_report(&spec_container, &kernel_config, &args.report_path);
            system_built = true;
            break;
        } else {
            // Some memory regions have had their physical address updated, rebuild the spec.
            iteration += 1;
        }
    }

    if !system_built {
        // Cannot build a reasonable spec, absurd.
        // Only reachable when there are setvar region_paddr that we keep selecting the wrong address.
        panic!("ERROR: fatal, failed to build system in {iteration} iterations");
    }

    Ok(())
}
