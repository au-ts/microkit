
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
pub enum RequestedImageType {
    Binary,
    Elf,
    Uimage,
    Unspecified,
}

impl RequestedImageType {
    fn parse(str: &str) -> Option<Self> {
        match str {
            "binary" => Some(RequestedImageType::Binary),
            "elf" => Some(RequestedImageType::Elf),
            "uimage" => Some(RequestedImageType::Uimage),
            _ => None,
        }
    }
}


#[derive(Debug,Clone)]
pub struct BuildConfig {
    pub sdf_path: PathBuf,
    pub board: String,
    pub config: String,
    pub report_path: PathBuf,
    pub capdl_json_path: Option<PathBuf>,
    pub output_path: PathBuf,
    pub search_paths: Vec<PathBuf>,
    pub requested_image_type: RequestedImageType,
}

#[derive(Debug)]
pub enum BuildConfigError {
    InvalidImageTypeParameter { parameter: String },
    InvalidBoardParameter { parameter: String },
    MissingParameter { parent_argument: &'static str },
    MissingRequiredArguments { args: Vec<&'static str> },
    UnrecognizedArgument { arg: String },
    HelpWanted,
}

impl fmt::Display for BuildConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidImageTypeParameter { parameter } => {
                write!(f, "argument --image-type: unknown parameter '{parameter}'")
            }
            Self::InvalidBoardParameter { parameter } => {
                write!(f, "argument --board: unknown parameter '{parameter}'")
            }
            Self::MissingParameter { parent_argument } => {
                write!(f, "argument {parent_argument}: expected one parameter")
            }
            Self::MissingRequiredArguments { args } => {
                write!(
                    f,
                    "the following arguments are required: {}",
                    args.join(", ")
                )
            }
            Self::UnrecognizedArgument { arg } => {
                write!(f, "unrecognized argument '{arg}'")
            }
            Self::HelpWanted => {
                write!(f, "printing help text")
            }
        }
    }
}

fn consume_parameter<I>(args: &mut I, argname: &'static str) -> Result<String, BuildConfigError>
where
    I: Iterator<Item = String>,
{
    args.next().ok_or(BuildConfigError::MissingParameter {parent_argument: argname})
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
    pub fn parse(args: &[String], available_boards: &[String]) -> Result<Self, BuildConfigError> {
        let mut args = args.iter().skip(1).cloned().peekable();

        let mut output_path = PathBuf::from("loader.img");
        let mut report_path = PathBuf::from("report.txt");
        let mut capdl_json_path = None;
        let mut search_paths = Vec::new();

        let mut sdf_path = None;
        let mut board = None;
        let mut config = None;
        let mut requested_image_type = RequestedImageType::Unspecified;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    return Err(BuildConfigError::HelpWanted);
                }
                "-o" | "--output" => {
                    output_path = consume_parameter(&mut args, "--output")?.into();
                }
                "-r" | "--report" => {
                    report_path = consume_parameter(&mut args, "--report")?.into();
                }
                "--board" => {
                    let board_param = consume_parameter(&mut args, "--board")?;
                    if !available_boards.contains(&board_param) {
                        return Err(BuildConfigError::InvalidBoardParameter {parameter: board_param});
                    }
                    board = Some(board_param);
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
                    match RequestedImageType::parse(value.as_str()) {
                        Some(image_type) => {
                            requested_image_type = image_type;
                        }
                        None => {
                            return Err( BuildConfigError::InvalidImageTypeParameter {parameter: value} );
                        }
                    }
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

        let mut missing_args = Vec::new();
        if board.is_none() {
            missing_args.push("--board");
        }
        if config.is_none() {
            missing_args.push("--config");
        }
        if sdf_path.is_none() {
            missing_args.push("system");
        }
        if !missing_args.is_empty() {
            return Err(BuildConfigError::MissingRequiredArguments {args: missing_args} );
        }
        let board = board.unwrap();
        let config = config.unwrap();
        let sdf_path = sdf_path.unwrap();

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
