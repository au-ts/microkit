//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

// we want our asserts, even if the compiler figures out they hold true already during compile-time
#![allow(clippy::assertions_on_constants)]

use microkit_tool::capdl::spec::ElfContent;
// use microkit_tool::capdl::untyped::InitSystem;
use microkit_tool::capdl::{build_capdl_spec, reserialise_spec};
use microkit_tool::elf::{ElfFile, ElfSegmentAttributes};
use microkit_tool::loader::Loader;
use microkit_tool::sdf::parse;
use microkit_tool::sel4::{
    emulate_kernel_boot, emulate_kernel_boot_partial, Arch, Config, PageSize, PlatformConfig,
    RiscvVirtualMemory,
};
use microkit_tool::util::{
    human_size_strict, json_str, json_str_as_bool, json_str_as_u64, round_up,
};
use microkit_tool::{serialise_ut, MemoryRegion};
use sel4_capdl_initializer_types::{ObjectNamesLevel, Spec};
use std::fs::{self, metadata};
use std::ops::Range;
use std::path::{Path, PathBuf};

// The capDL initialiser heap size is calculated by:
// (spec size * INITIALISER_HEAP_MULTIPLIER) + INITIALISER_HEAP_ADD_ON_CONSTANT
const INITIALISER_HEAP_MULTIPLIER: f64 = 2.0;
const INITIALISER_HEAP_ADD_ON_CONSTANT: u64 = 16 * 4096; // 64kb

fn get_full_path(path: &Path, search_paths: &Vec<PathBuf>) -> Option<PathBuf> {
    for search_path in search_paths {
        let full_path = search_path.join(path);
        if full_path.exists() {
            return Some(full_path.to_path_buf());
        }
    }

    None
}

fn print_usage() {
    println!("usage: microkit [-h] [-o OUTPUT] [-r REPORT] --board BOARD --config CONFIG [--search-path [SEARCH_PATH ...]] system")
}

fn print_help(available_boards: &[String]) {
    print_usage();
    println!("\npositional arguments:");
    println!("  system");
    println!("\noptions:");
    println!("  -h, --help, show this help message and exit");
    println!("  -o, --output OUTPUT");
    println!("  -r, --report REPORT");
    println!("  --board {}", available_boards.join("\n          "));
    println!("  --config CONFIG");
    println!("  --search-path [SEARCH_PATH ...]");
}

struct Args<'a> {
    system: &'a str,
    board: &'a str,
    config: &'a str,
    report: &'a str,
    output: &'a str,
    search_paths: Vec<&'a String>,
    initialiser_heap_size_multiplier: f64,
}

impl<'a> Args<'a> {
    pub fn parse(args: &'a [String], available_boards: &[String]) -> Args<'a> {
        // Default arguments
        let mut output = "loader.img";
        let mut report = "report.txt";
        let mut search_paths = Vec::new();
        // Arguments expected to be provided by the user
        let mut system = None;
        let mut board = None;
        let mut config = None;
        let mut initialiser_heap_size_multiplier = INITIALISER_HEAP_MULTIPLIER;

        if args.len() <= 1 {
            print_usage();
            std::process::exit(1);
        }

        let mut i = 1;
        let mut unknown = vec![];
        let mut in_search_path = false;
        while i < args.len() {
            match args[i].as_str() {
                "-h" | "--help" => {
                    print_help(available_boards);
                    std::process::exit(0);
                }
                "-o" | "--output" => {
                    in_search_path = false;
                    if i < args.len() - 1 {
                        output = &args[i + 1];
                        i += 1;
                    } else {
                        eprintln!("microkit: error: argument -o/--output: expected one argument");
                        std::process::exit(1);
                    }
                }
                "-r" | "--report" => {
                    in_search_path = false;
                    if i < args.len() - 1 {
                        report = &args[i + 1];
                        i += 1;
                    } else {
                        eprintln!("microkit: error: argument -r/--report: expected one argument");
                        std::process::exit(1);
                    }
                }
                "--board" => {
                    in_search_path = false;
                    if i < args.len() - 1 {
                        board = Some(&args[i + 1]);
                        i += 1;
                    } else {
                        eprintln!("microkit: error: argument --board: expected one argument");
                        std::process::exit(1);
                    }
                }
                "--config" => {
                    in_search_path = false;
                    if i < args.len() - 1 {
                        config = Some(&args[i + 1]);
                        i += 1;
                    } else {
                        eprintln!("microkit: error: argument --config: expected one argument");
                        std::process::exit(1);
                    }
                }
                "-x" | "--initialiser_heap_size_multiplier" => {
                    in_search_path = false;
                    if i < args.len() - 1 {
                        match args[i + 1].parse::<f64>() {
                            Ok(multiplier) => initialiser_heap_size_multiplier = multiplier,
                            Err(e) => {
                                eprintln!("microkit: error: argument --initialiser_heap_size_multiplier: failed to parse as float: {}", e);
                                std::process::exit(1);
                            }
                        }
                        i += 1;
                    } else {
                        eprintln!("microkit: error: argument --initialiser_heap_size_multiplier: expected one argument");
                        std::process::exit(1);
                    }
                }
                "--search-path" => {
                    in_search_path = true;
                }
                _ => {
                    if in_search_path {
                        search_paths.push(&args[i]);
                    } else if system.is_none() {
                        system = Some(&args[i]);
                    } else {
                        // This call to clone is okay since having unknown
                        // arguments is rare.
                        unknown.push(args[i].clone());
                    }
                }
            }

            i += 1;
        }

        if !unknown.is_empty() {
            print_usage();
            eprintln!(
                "microkit: error: unrecognised arguments: {}",
                unknown.join(" ")
            );
            std::process::exit(1);
        }

        let mut missing_args = Vec::new();
        if board.is_none() {
            missing_args.push("--board");
        }
        if config.is_none() {
            missing_args.push("--config");
        }
        if system.is_none() {
            missing_args.push("system");
        }

        if !missing_args.is_empty() {
            print_usage();
            eprintln!(
                "microkit: error: the following arguments are required: {}",
                missing_args.join(", ")
            );
            std::process::exit(1);
        }

        Args {
            system: system.unwrap(),
            board: board.unwrap(),
            config: config.unwrap(),
            report,
            output,
            search_paths,
            initialiser_heap_size_multiplier,
        }
    }
}

fn main() -> Result<(), String> {
    let exe_path = std::env::current_exe().unwrap();
    let sdk_env = std::env::var("MICROKIT_SDK");
    let sdk_dir = match sdk_env {
        Ok(ref value) => Path::new(value),
        Err(err) => match err {
            // If there is no MICROKIT_SDK explicitly set, use the one that the binary is in.
            std::env::VarError::NotPresent => exe_path.parent().unwrap().parent().unwrap(),
            _ => {
                return Err(format!(
                    "Could not read MICROKIT_SDK environment variable: {}",
                    err
                ))
            }
        },
    };

    if !sdk_dir.exists() {
        eprintln!(
            "Error: SDK directory '{}' does not exist.",
            sdk_dir.display()
        );
        std::process::exit(1);
    }

    let boards_path = sdk_dir.join("board");
    if !boards_path.exists() || !boards_path.is_dir() {
        eprintln!(
            "Error: SDK directory '{}' does not have a 'board' sub-directory.",
            sdk_dir.display()
        );
        std::process::exit(1);
    }

    let mut available_boards = Vec::new();
    for p in fs::read_dir(&boards_path).unwrap() {
        let path_buf = p.unwrap().path();
        let path = path_buf.as_path();
        if path.is_dir() {
            available_boards.push(path.file_name().unwrap().to_str().unwrap().to_string());
        }
    }
    available_boards.sort();

    let env_args: Vec<_> = std::env::args().collect();
    let args = Args::parse(&env_args, &available_boards);

    let board_path = boards_path.join(args.board);
    if !board_path.exists() {
        eprintln!(
            "Error: board path '{}' does not exist.",
            board_path.display()
        );
        std::process::exit(1);
    }

    let mut available_configs = Vec::new();
    for p in fs::read_dir(board_path).unwrap() {
        let path_buf = p.unwrap().path();
        let path = path_buf.as_path();

        if path.file_name().unwrap() == "example" {
            continue;
        }

        if path.is_dir() {
            available_configs.push(path.file_name().unwrap().to_str().unwrap().to_string());
        }
    }

    if !available_configs.contains(&args.config.to_string()) {
        eprintln!(
            "microkit: error: argument --config: invalid choice: '{}' (choose from: {})",
            args.config,
            available_configs.join(", ")
        )
    }

    let elf_path = sdk_dir
        .join("board")
        .join(args.board)
        .join(args.config)
        .join("elf");
    let loader_elf_path = elf_path.join("loader.elf");
    let kernel_elf_path = elf_path.join("sel4.elf");
    let monitor_elf_path = elf_path.join("monitor.elf");
    let capdl_init_elf_path = elf_path.join("capdl_initialiser.elf");

    let kernel_config_path = sdk_dir
        .join("board")
        .join(args.board)
        .join(args.config)
        .join("include/kernel/gen_config.json");

    let invocations_all_path = sdk_dir
        .join("board")
        .join(args.board)
        .join(args.config)
        .join("invocations_all.json");

    if !elf_path.exists() {
        eprintln!(
            "Error: board ELF directory '{}' does not exist",
            elf_path.display()
        );
        std::process::exit(1);
    }
    if !kernel_elf_path.exists() {
        eprintln!(
            "Error: kernel ELF '{}' does not exist",
            kernel_elf_path.display()
        );
        std::process::exit(1);
    }
    if !monitor_elf_path.exists() {
        eprintln!(
            "Error: monitor ELF '{}' does not exist",
            monitor_elf_path.display()
        );
        std::process::exit(1);
    }
    if !capdl_init_elf_path.exists() {
        eprintln!(
            "Error: CapDL initialiser ELF '{}' does not exist",
            capdl_init_elf_path.display()
        );
        std::process::exit(1);
    }
    if !kernel_config_path.exists() {
        eprintln!(
            "Error: kernel configuration file '{}' does not exist",
            kernel_config_path.display()
        );
        std::process::exit(1);
    }
    if !invocations_all_path.exists() {
        eprintln!(
            "Error: invocations JSON file '{}' does not exist",
            invocations_all_path.display()
        );
        std::process::exit(1);
    }

    let system_path = Path::new(args.system);
    if !system_path.exists() {
        eprintln!(
            "Error: system description file '{}' does not exist",
            system_path.display()
        );
        std::process::exit(1);
    }

    let xml: String = fs::read_to_string(args.system).unwrap();

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

    let (device_regions, normal_regions) = match arch {
        Arch::X86_64 => (None, None),
        _ => {
            let kernel_platform_config_path = sdk_dir
                .join("board")
                .join(args.board)
                .join(args.config)
                .join("platform_gen.json");

            if !kernel_platform_config_path.exists() {
                eprintln!(
                    "Error: kernel platform configuration file '{}' does not exist",
                    kernel_platform_config_path.display()
                );
                std::process::exit(1);
            }

            let kernel_platform_config: PlatformConfig =
                serde_json::from_str(&fs::read_to_string(kernel_platform_config_path).unwrap())
                    .unwrap();

            (
                Some(kernel_platform_config.devices),
                Some(kernel_platform_config.memory),
            )
        }
    };

    let hypervisor = match arch {
        Arch::Aarch64 => json_str_as_bool(&kernel_config_json, "ARM_HYPERVISOR_SUPPORT")?,
        // Hypervisor mode is not available on RISC-V and x86_64
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
        _ => None,
    };

    let arm_smc = match arch {
        Arch::Aarch64 => Some(json_str_as_bool(&kernel_config_json, "ALLOW_SMC_CALLS")?),
        _ => None,
    };

    let x86_xsave_size = match arch {
        Arch::X86_64 => Some(json_str_as_u64(&kernel_config_json, "XSAVE_SIZE")? as usize),
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
        minimum_page_size: 4096,
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
        benchmark: args.config == "benchmark",
        fpu: json_str_as_bool(&kernel_config_json, "HAVE_FPU")?,
        arm_pa_size_bits,
        arm_smc,
        riscv_pt_levels: Some(RiscvVirtualMemory::Sv39),
        x86_xsave_size,
        invocations_labels,
        device_regions,
        normal_regions,
    };

    if kernel_config.arch != Arch::X86_64 && !loader_elf_path.exists() {
        eprintln!(
            "Error: loader ELF '{}' does not exist",
            loader_elf_path.display()
        );
        std::process::exit(1);
    }

    if let Arch::Aarch64 = kernel_config.arch {
        assert!(
            kernel_config.hypervisor,
            "Microkit tool expects a kernel with hypervisor mode enabled on AArch64."
        );
    }

    assert!(
        kernel_config.word_size == 64,
        "Microkit tool has various assumptions about the word size being 64-bits."
    );

    let system = match parse(args.system, &xml, &kernel_config) {
        Ok(system) => system,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    let monitor_elf = ElfFile::from_path(&monitor_elf_path)?;

    let mut search_paths = vec![std::env::current_dir().unwrap()];
    for path in args.search_paths {
        search_paths.push(PathBuf::from(path));
    }

    // Get the elf files for each pd:
    let mut pd_elf_files = Vec::with_capacity(system.protection_domains.len());
    for pd in &system.protection_domains {
        match get_full_path(&pd.program_image, &search_paths) {
            Some(path) => {
                pd_elf_files.push(ElfFile::from_path(&path)?);
            }
            None => {
                return Err(format!(
                    "unable to find program image: '{}'",
                    pd.program_image.display()
                ))
            }
        }
    }
    pd_elf_files.push(monitor_elf);

    // We have parsed the XML and all ELF files, create the CapDL spec of the system described in the XML.
    let spec = build_capdl_spec(&kernel_config, &mut pd_elf_files, &system)?;

    // Reserialise the spec into a type that can be understood by rust-sel4.
    // @billn improve? Instead of this serialise our spec type -> deserialise into their spec type -> serialise business, why dont we just serialise
    // our Spec type and let the run time initialiser deal with the type conversion?
    let spec_reserialised = {
        // Eagerly write out the spec so we can debug in case something crash later.
        let spec_as_json = serde_json::to_string(&spec).unwrap();
        fs::write(args.report, &spec_as_json).unwrap();

        // The full type definition is `Spec<'a, N, D, M>` where:
        // N = object name type
        // D = frame fill data type
        // M = embedded frame data type
        // Only N and D is useful for Microkit.
        serde_json::from_str::<Spec<String, ElfContent, ()>>(&spec_as_json).unwrap()
    };

    // Now embed the built spec into the CapDL initialiser.
    let name_level = match args.config {
        "debug" => ObjectNamesLevel::All,
        // We don't copy over the object names as there is no printing in these configuration to save memory.
        "release" | "benchmark" => ObjectNamesLevel::None,
        _ => panic!("unknown configuration {}", args.config),
    };

    let num_objects = spec.objects.len();
    let capdl_spec_as_binary =
        reserialise_spec::reserialise_spec(&pd_elf_files, &spec_reserialised, &name_level);

    let footprint = capdl_spec_as_binary.len();
    let heap_size = round_up(
        (footprint as f64 * args.initialiser_heap_size_multiplier) as u64
            + INITIALISER_HEAP_ADD_ON_CONSTANT,
        PageSize::Small as u64,
    ) as usize;

    // Patch the spec and heap into the ELF image. These symbol names must match
    // rust-sel4/crates/sel4-capdl-initializer/src/main.rs
    let mut initialiser_elf = ElfFile::from_path(&capdl_init_elf_path)?;
    let spec_vaddr = round_up(initialiser_elf.next_vaddr(), PageSize::Small as u64);
    initialiser_elf.add_segment(ElfSegmentAttributes::Read, spec_vaddr, capdl_spec_as_binary);
    initialiser_elf.write_symbol(
        "sel4_capdl_initializer_serialized_spec_start",
        &spec_vaddr.to_le_bytes(),
    )?;
    initialiser_elf.write_symbol(
        "sel4_capdl_initializer_serialized_spec_size",
        &footprint.to_le_bytes(),
    )?;

    let heap_vaddr = round_up(initialiser_elf.next_vaddr(), PageSize::Small as u64);
    initialiser_elf.add_segment(
        ElfSegmentAttributes::Read | ElfSegmentAttributes::Write,
        heap_vaddr,
        vec![0; heap_size],
    );
    initialiser_elf.write_symbol(
        "sel4_capdl_initializer_heap_start",
        &heap_vaddr.to_le_bytes(),
    )?;
    initialiser_elf.write_symbol(
        "sel4_capdl_initializer_heap_size",
        &(heap_vaddr + heap_size as u64).to_le_bytes(),
    )?;

    initialiser_elf.write_symbol(
        "sel4_capdl_initializer_image_start",
        &initialiser_elf.lowest_vaddr().to_le_bytes(),
    )?;
    initialiser_elf.write_symbol(
        "sel4_capdl_initializer_image_end",
        &initialiser_elf.highest_vaddr().to_le_bytes(),
    )?;

    println!(
        "CAPDL SPEC: number of root objects = {}, spec footprint = {}, initialiser heap size = {}",
        num_objects,
        human_size_strict(footprint as u64),
        human_size_strict(heap_size as u64)
    );
    let initialiser_highest_vaddr_rounded =
        round_up(initialiser_elf.highest_vaddr(), PageSize::Small as u64);
    let initialiser_vaddr_range =
        Range::from(initialiser_elf.lowest_vaddr()..initialiser_highest_vaddr_rounded);
    println!(
        "INITIAL TASK: size = {}, vaddr = [0x{:x}..0x{:x}], entry = 0x{:x}",
        human_size_strict(initialiser_vaddr_range.end - initialiser_vaddr_range.start),
        initialiser_vaddr_range.start,
        initialiser_vaddr_range.end,
        initialiser_elf.entry
    );

    // For x86 we write out the initialiser ELF as is, but on ARM and RISC-V we build the bootloader image.
    if kernel_config.arch == Arch::X86_64 {
        initialiser_elf.reserialise(Path::new(args.output))?;
    } else {
        // Now that we have the entire spec and CapDL initialiser ELF with embedded spec,
        // we can determine exactly how much memory will be available statically when the kernel
        // drops to userspace on ARM and RISC-V. This allow us to sanity check that:
        // 1. There are enough memory to allocate all the objects required in the spec.
        // 2. All frames with a physical attached reside in legal memory (device or normal).
        // 3. Objects can be allocated from the free untyped list. For example, we detect
        //    situations where you might have a few frames with size bit 12 to allocate but
        //    only have untyped with size bit <12 remaining.

        // We achieve this by emulating the kernel's boot process in the tool:

        // Determine how much memory the CapDL initialiser needs.
        let initial_task_size = initialiser_vaddr_range.end - initialiser_vaddr_range.start;

        // Parse the kernel's ELF to determine the kernel's window.
        let kernel_elf = ElfFile::from_path(&kernel_elf_path).unwrap();

        // Now determine how much memory we have after the kernel boots.
        let (mut available_memory, kernel_boot_region) =
            emulate_kernel_boot_partial(&kernel_config, &kernel_elf);

        // The kernel relies on the initial task region being allocated above the kernel
        // boot/ELF region, so we have the end of the kernel boot region as the lower
        // bound for allocating the reserved region.
        let initial_task_phys_base =
            available_memory.allocate_from(initial_task_size, kernel_boot_region.end);

        let initial_task_phys_region = MemoryRegion::new(
            initial_task_phys_base,
            initial_task_phys_base + initial_task_size,
        );
        let initial_task_virt_region = MemoryRegion::new(
            initialiser_elf.lowest_vaddr(),
            initialiser_highest_vaddr_rounded,
        );

        // With the initial task region determined the kernel boot can be emulated. This provides
        // the boot info information which is needed for the next steps
        let kernel_boot_info = emulate_kernel_boot(
            &kernel_config,
            &kernel_elf,
            initial_task_phys_region,
            initial_task_virt_region,
        );

        // We got the untypeds list, now follow the CapDL object allocation algorithm to catch
        // issues at build time.
        // Step 1: sort untypeds by paddr.
        let mut untypeds_by_paddr = kernel_boot_info.untyped_objects.clone();
        untypeds_by_paddr.sort_by_key(|ut| ut.base());

        // Step 2: create object "windows" for objects that doesn't specify paddr,
        // where each window contains all objects of the array index size bits.
        let mut object_windows_by_size: Vec<Option<Range<usize>>> =
            vec![None; kernel_config.word_size as usize];
        let first_obj_id_without_paddr = spec
            .objects
            .partition_point(|named_obj| named_obj.object.paddr().is_some());
        for (id, named_object) in spec.objects[first_obj_id_without_paddr..]
            .iter()
            .enumerate()
        {
            let phys_size_bit = named_object.object.physical_size_bits(&kernel_config) as usize;
            if phys_size_bit > 0 {
                let window_maybe = object_windows_by_size.get_mut(phys_size_bit).unwrap();
                match window_maybe {
                    Some(window) => window.end += 1,
                    None => {
                        let _ = window_maybe.insert(Range::from(
                            first_obj_id_without_paddr + id..first_obj_id_without_paddr + id + 1,
                        ));
                    }
                }
            }
        }

        // Step 3: Sanity check that all objects with a paddr attached can be allocated.
        let mut phys_addrs_ok = true;
        for obj_with_paddr_id in 0..first_obj_id_without_paddr {
            let named_obj = spec.objects.get(obj_with_paddr_id).unwrap();
            let paddr_base = named_obj.object.paddr().unwrap() as u64;

            let obj_size_bytes = 1 << named_obj.object.physical_size_bits(&kernel_config);
            let paddr_range = Range::from(paddr_base..paddr_base + obj_size_bytes);

            // Binary search for the untyped that would fit, if we can't find one, this object is not in valid memory.
            let mut low = 0;
            let mut high = untypeds_by_paddr.len();
            let mut found = false;
            while low < high {
                let mid = low + (high - low) / 2;
                let candidate_ut = untypeds_by_paddr.get(mid).unwrap();

                if paddr_range.start >= candidate_ut.end() {
                    low = mid + 1;
                } else if paddr_range.start < candidate_ut.base() {
                    high = mid;
                } else if paddr_range.start >= candidate_ut.base() {
                    if paddr_range.end <= candidate_ut.end() {
                        // Object paddr range doesn't span across 2 untypeds, all good.
                        found = true;
                    }
                    break;
                }
            }

            if !found {
                eprintln!("Error: object '{}', with paddr 0x{:0>12x}..0x{:0>12x} is not in any valid memory region.", named_obj.name, paddr_range.start, paddr_range.end);
                phys_addrs_ok = false;
            }
        }

        if !phys_addrs_ok {
            eprintln!("Below are the valid ranges of memory to be allocated from:");
            eprintln!("Valid ranges outside of main memory:");
            for ut in untypeds_by_paddr.iter().filter(|ut| ut.is_device) {
                eprintln!("     [0x{:0>12x}..0x{:0>12x})", ut.base(), ut.end());
            }
            eprintln!("Valid ranges within main memory:");
            for ut in untypeds_by_paddr.iter().filter(|ut| !ut.is_device) {
                eprintln!("     [0x{:0>12x}..0x{:0>12x})", ut.base(), ut.end());
            }
            std::process::exit(1);
        }

        // Step 4: now simulate the allocations
        let num_objs_with_paddr = first_obj_id_without_paddr;
        let mut next_obj_id_with_paddr = 0;
        for ut in untypeds_by_paddr.iter() {
            let mut cur_paddr = ut.base();

            // println!(
            //     "TRACE [sel4_capdl_initializer_core] Allocating from untyped: {:#x}..{:#x} (size_bits = {}, device = {:?})",
            //     ut.base(),
            //     ut.end(),
            //     ut.size_bits(),
            //     ut.is_device
            // );

            loop {
                // If this untyped covers frames that specify a paddr, don't allocate ordinary objects
                // past the lowest frame's paddr.
                let target = if next_obj_id_with_paddr < num_objs_with_paddr {
                    ut.end().min(
                        spec.objects
                            .get(next_obj_id_with_paddr)
                            .unwrap()
                            .object
                            .paddr()
                            .unwrap() as u64,
                    )
                } else {
                    ut.end()
                };
                let target_is_obj_with_paddr = target < ut.end();

                while cur_paddr < target {
                    let max_size_bits = usize::try_from(cur_paddr.trailing_zeros())
                        .unwrap()
                        .min((target - cur_paddr).trailing_zeros().try_into().unwrap());
                    let mut created = false;

                    // If this UT is in main memory, allocate all the objects that does not specify a paddr first.
                    if !ut.is_device {
                        // Greedily create a largest possible objects that would fit in this untyped.
                        // If at the current size we cannot allocate any more object, drop to objects of smaller
                        // size that still need to be allocated.
                        for size_bits in (0..=max_size_bits).rev() {
                            let obj_id_range_maybe =
                                object_windows_by_size.get_mut(size_bits).unwrap();
                            if obj_id_range_maybe.is_some() {
                                // Got objects at this size bits, check if we still have any to allocate
                                if obj_id_range_maybe.as_ref().unwrap().start
                                    < obj_id_range_maybe.as_ref().unwrap().end
                                {
                                    let named_obj = spec
                                        .get_root_object(obj_id_range_maybe.as_ref().unwrap().start)
                                        .unwrap();

                                    // println!("TRACE [sel4_capdl_initializer_core] Creating kernel object: paddr=0x{:x}, size_bits={} name={:?}", cur_paddr,named_obj.object.physical_size_bits(&kernel_config), named_obj.name);

                                    cur_paddr +=
                                        1 << named_obj.object.physical_size_bits(&kernel_config);
                                    obj_id_range_maybe.as_mut().unwrap().start += 1;
                                    created = true;
                                    break;
                                }
                            }
                        }
                    }
                    if !created {
                        if target_is_obj_with_paddr {
                            // Manipulate the untyped's watermark to allocate at the correct paddr.
                            println!(
                                // "TRACE [sel4_capdl_initializer_core] Creating dummy: paddr=0x{cur_paddr:x}, size_bits={max_size_bits}"
                            );

                            cur_paddr += 1 << max_size_bits;
                        } else {
                            cur_paddr = target;
                        }
                    }
                }
                if target_is_obj_with_paddr {
                    // Watermark now at the correct level, make the actual object
                    let named_obj = spec.get_root_object(next_obj_id_with_paddr).unwrap();

                    // println!(
                    //     "TRACE [sel4_capdl_initializer_core] Creating device object: paddr=0x{:x}, size_bits={} name={:?}",
                    //     cur_paddr,
                    //     named_obj.object.physical_size_bits(&kernel_config),
                    //     named_obj.name
                    // );

                    cur_paddr += 1 << named_obj.object.physical_size_bits(&kernel_config);
                    next_obj_id_with_paddr += 1;
                } else {
                    break;
                }
            }
        }

        // Ensure that we've created every objects
        let mut oom = false;
        for size_bit in 0..kernel_config.word_size {
            let obj_id_range_maybe = object_windows_by_size.get(size_bit as usize).unwrap();
            if obj_id_range_maybe.is_some() {
                let obj_id_range = obj_id_range_maybe.as_ref().unwrap();
                if obj_id_range.start != obj_id_range.end {
                    oom = true;
                    let shortfall = (obj_id_range.end - obj_id_range.start) as u64;
                    let individual_sz = (1 << size_bit) as u64;
                    eprintln!("Error: ran out of untypeds for allocating objects of size {}, still need to create {} objects which requires {} of additional memory.", human_size_strict(individual_sz), shortfall, human_size_strict(individual_sz * shortfall));
                }
            }
        }
        if oom {
            eprintln!("Out of untypeds. Please see the report for more details.");
            std::process::exit(1);
        }

        // Everything checks out, patch the list of untypeds we used to simulate object allocation into the initialiser.
        // At runtime the intialiser will validate what we simulated against what the kernel gives it. If they deviate
        // we will have problems! For example, if we simulated with more memory than what's actually available, the initialiser
        // can crash.
        let mut uts_desc: Vec<u8> = Vec::new();
        for ut in kernel_boot_info.untyped_objects.iter() {
            uts_desc.extend(serialise_ut(ut));
        }

        initialiser_elf.write_symbol(
            "sel4_capdl_initializer_expected_untypeds_list_num_entries",
            &(kernel_boot_info.untyped_objects.len() as u64).to_le_bytes(),
        )?;
        initialiser_elf.write_symbol("sel4_capdl_initializer_expected_untypeds_list", &uts_desc)?;

        // Everything checks out, now build the bootloader!
        let loader = Loader::new(
            &kernel_config,
            Path::new(&loader_elf_path),
            &kernel_elf,
            &initialiser_elf,
            initial_task_phys_base,
            initialiser_vaddr_range,
        );

        loader.write_image(Path::new(args.output));

        println!(
            "LOADER: image size = {}",
            human_size_strict(metadata(args.output).unwrap().len())
        );
    }

    Ok(())
}
