
//
// Copyright 2025, UNSW
//
// SPDX-License-Identifier: BSD-2-Clause
//

// we want our asserts, even if the compiler figures out they hold true already during compile-time
#![allow(clippy::assertions_on_constants)]

use std::iter::Peekable;
use std::path::PathBuf;
use std::fmt;


fn print_usage() {
    println!("usage: microkit [-h] [-o OUTPUT] [--image-type {{binary,elf,uimage}}] [-r REPORT] --board BOARD --config CONFIG [--capdl-json CAPDL_SPEC] --search-path [SEARCH_PATH ...] system")
}

fn print_help(available_boards: &[String]) {
    print_usage();
    println!("\npositional arguments:");
    println!("  system");
    println!("\noptions:");
    println!("  -h, --help, show this help message and exit");
    println!("  -o, --output OUTPUT");
    println!("  -r, --report REPORT");
    println!("  --image-type {{binary,elf,uimage}}");
    println!("  --board {}", available_boards.join("\n          "));
    println!("  --config CONFIG");
    println!("  --capdl-json CAPDL_SPEC (JSON format)");
    println!("  --search-path [SEARCH_PATH ...]");
}

#[derive(Debug,Clone)]
struct BuildConfig {
    sdf_path: PathBuf,
    board: String,
    config: String,
    report_path: PathBuf,
    capdl_json_path: Option<PathBuf>,
    output_path: PathBuf,
    search_paths: Vec<PathBuf>,
    requested_image_type: Option<String>,
}

#[derive(Debug)]
enum BuildConfigError {
    InvalidImageTypeParameter { parameter: String },
    MissingParameter { parentArgument: &'static str },
    MissingRequiredArgument { arg: &'static str },
    UnrecognizedArgument { arg: String },
}

impl fmt::Display for BuildConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidImageTypeParameter { parameter } => {
                write!(f, "argument --image-type: unknown parameter '{parameter}'")
            }
            Self::MissingParameter { parentArgument } => {
                write!(f, "argument {parentArgument}: expected one parameter")
            }
            Self::MissingRequiredArgument { arg } => {
                write!(f, "missing required argument '{arg}'")
            }
            Self::UnrecognizedArgument { arg } => {
                write!(f, "unrecognised argument '{arg}'")
            }
        }
    }
}

fn consume_parameter<I>(args: &mut I, argname: &'static str) -> Result<String, BuildConfigError>
where
    I: Iterator<Item = String>,
{
    args.next().ok_or(BuildConfigError::MissingParameter {parentArgument: argname})
}

fn consume_parameters<I>(args: &mut Peekable<I>) -> Vec<String>
where
    I: Iterator<Item = String>,
{
    let mut values = Vec::new();
    while let Some(next) = args.peek() {
        if next.starts_with("-") { break; }
        if let Some(next) = args.next() { values.push(next) };
    }
    values
}

impl BuildConfig {
    fn parse(args: &[String], available_boards: &[String]) -> Result<Self, BuildConfigError> {
        let mut args = args.iter().skip(1).cloned().peekable();

        let mut output_path = PathBuf::from("loader.img");
        let mut report_path = PathBuf::from("report.txt");
        let mut capdl_json_path = None;
        let mut search_paths = Vec::new();

        let mut sdf_path = None;
        let mut board = None;
        let mut config = None;
        let mut requested_image_type = None;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    print_help(available_boards);
                    std::process::exit(0);
                }
                "-o" | "--output" => {
                    output_path = consume_parameter(&mut args, "--output")?.into();
                }
                "-r" | "--report" => {
                    report_path = consume_parameter(&mut args, "--report")?.into();
                }
                "--board" => {
                    board = Some(consume_parameter(&mut args, "--board")?);
                }
                "--config" => {
                    config = Some(consume_parameter(&mut args, "--config")?);
                }
                "--capdl-json" => {
                    capdl_json_path = Some(consume_parameter(&mut args, "--capdl-json")?.into());
                }
                "--search-path" => {
                    let params = consume_parameters(&mut args);
                    search_paths.extend(params.into_iter().map(PathBuf::from));
                }
                "--image-type" => {
                    let value = consume_parameter(&mut args, "--image-type")?;
                    requested_image_type = Some(value);
                }
                value => {
                    if sdf_path.is_none() {
                        sdf_path = Some(value.into());
                    } else {
                        return Err( BuildConfigError::UnrecognizedArgument {arg: value.to_owned()} );
                    }
                }
            }
        }
        let sdf_path = sdf_path.ok_or(BuildConfigError::MissingRequiredArgument {arg: "sdf path"})?;
        let board = board.ok_or(BuildConfigError::MissingRequiredArgument {arg: "--board"})?;
        let config = config.ok_or(BuildConfigError::MissingRequiredArgument {arg: "--config"})?;
        Ok(Self {
            sdf_path,
            board,
            config,
            report_path,
            capdl_json_path,
            output_path,
            search_paths,
            requested_image_type,
        })
    }
}

pub struct Args<'a> {
    pub system: &'a str,
    pub board: &'a str,
    pub config: &'a str,
    pub report: &'a str,
    pub capdl_json: Option<&'a str>,
    pub output: &'a str,
    pub search_paths: Vec<&'a String>,
    pub output_image_type: Option<&'a str>,
}

impl<'a> Args<'a> {
    pub fn parse(args: &'a [String], available_boards: &[String]) -> Args<'a> {
        // Default arguments
        let mut output = "loader.img";
        let mut report = "report.txt";
        let mut capdl_json = None;
        let mut search_paths = Vec::new();
        // Arguments expected to be provided by the user
        let mut system = None;
        let mut board = None;
        let mut config = None;
        let mut output_image_type = None;

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
                "--capdl-json" => {
                    in_search_path = false;
                    if i < args.len() - 1 {
                        capdl_json = Some(args[i + 1].as_str());
                        i += 1;
                    } else {
                        eprintln!("microkit: error: argument --capdl-json: expected one argument");
                        std::process::exit(1);
                    }
                }
                "--search-path" => {
                    in_search_path = true;
                }
                "--image-type" => {
                    if i < args.len() - 1 {
                        output_image_type = Some(args[i + 1].as_str());
                        i += 1;
                    } else {
                        eprintln!("microkit: error: argument --image-type: expected one argument");
                        std::process::exit(1);
                    }
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
            capdl_json,
            output,
            search_paths,
            output_image_type,
        }
    }
}
