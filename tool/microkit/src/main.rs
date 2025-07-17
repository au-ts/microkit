//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

// we want our asserts, even if the compiler figures out they hold true already during compile-time
#![allow(clippy::assertions_on_constants)]

use microkit_tool::capdl::spec::BytesContent;
use microkit_tool::elf::ElfFile;
// use loader::Loader;
use microkit_tool::capdl::{build_capdl_spec, render_elf, reserialize_spec};
use microkit_tool::sdf::{
    parse,
};
use microkit_tool::sel4::{
    Arch, Config, PageSize, RiscvVirtualMemory
};
use sel4_capdl_initializer_types::{ObjectNamesLevel, Spec};
use std::cell::RefCell;
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use microkit_tool::util::{
    json_str, json_str_as_bool, json_str_as_u64,
};

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
        Arch::X86_64 => Some(json_str_as_u64(&kernel_config_json, "XSAVE_SIZE").unwrap() as usize),
        _ => None
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
        hypervisor,
        benchmark: args.config == "benchmark",
        fpu: json_str_as_bool(&kernel_config_json, "HAVE_FPU")?,
        arm_pa_size_bits,
        arm_smc,
        riscv_pt_levels: Some(RiscvVirtualMemory::Sv39),
        x86_xsave_size,
        invocations_labels,
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

    let monitor_elf: Rc<RefCell<ElfFile>> = Rc::new(ElfFile::from_path(&monitor_elf_path).unwrap().into());

    let mut search_paths = vec![std::env::current_dir().unwrap()];
    for path in args.search_paths {
        search_paths.push(PathBuf::from(path));
    }

    // Get the elf files for each pd:
    let mut pd_elf_files = Vec::with_capacity(system.protection_domains.len());
    for pd in &system.protection_domains {
        match get_full_path(&pd.program_image, &search_paths) {
            Some(path) => {
                let elf: Rc<RefCell<ElfFile>> = Rc::new(ElfFile::from_path(&path).unwrap().into());
                pd_elf_files.push(elf);
            }
            None => {
                return Err(format!(
                    "unable to find program image: '{}'",
                    pd.program_image.display()
                ))
            }
        }
    }

    // We have parsed the XML and all ELF files, create the CapDL spec of the system described in the XML.
    let spec = build_capdl_spec(&kernel_config, monitor_elf, &mut pd_elf_files, &system)?;

    // Eagerly write out the spec so we can debug in case something crash later.
    let spec_as_json = serde_json::to_string_pretty(&spec).unwrap();
    fs::write(args.report, &spec_as_json).unwrap();

    // Reserialise the spec into a type that can be understood by rust-sel4.
    let spec_reserialised = serde_json::from_str::<Spec<String, BytesContent, ()>>(&spec_as_json).unwrap();

    // Frame size bits for embedding into the CapDL spec from ELFs.
    // MUST match up with the frame size when creating ELF specs.
    let granule_size_bits = PageSize::Small.fixed_size_bits(&kernel_config) as usize;

    // Now embed the built spec into the CapDL initialiser.
    // A re-implementation of dep/rust-sel4/crates/sel4-capdl-initializer/add-spec/src/main.rs
    // so we don't have to call out to it as a subprocess.
    let serialized_spec = reserialize_spec::reserialize_spec(
        &spec_reserialised,
        &ObjectNamesLevel::All,
    );

    let footprint = serialized_spec.len();
    let heap_size = footprint * 2 + 16 * 4096;

    let render_elf_args = render_elf::RenderElfArgs {
        data: &serialized_spec,
        granule_size_bits: granule_size_bits,
        heap_size,
    };

    // Patch the CapDL initialiser ELF with the spec and write it out.
    let initializer_elf_buf = fs::read(capdl_init_elf_path).unwrap();
    let rendered_initializer_elf_buf = match object::File::parse(&*initializer_elf_buf).unwrap() {
        object::File::Elf32(initializer_elf) => render_elf_args.call_with(&initializer_elf),
        object::File::Elf64(initializer_elf) => render_elf_args.call_with(&initializer_elf),
        _ => {
            panic!()
        }
    };

    fs::write(args.output, rendered_initializer_elf_buf).unwrap();

    Ok(())
}
