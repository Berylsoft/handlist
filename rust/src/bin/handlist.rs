//! `handlist`: check and format handlist files.

use argh::FromArgs;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const SKIPPED_DIRECTORIES: &[&str] = &[".git", "target", "node_modules"];

#[derive(FromArgs)]
/// Check and format handlist files.
struct Handlist {
    #[argh(subcommand)]
    command: Command,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum Command {
    Check(CheckArgs),
    Fmt(FmtArgs),
}

#[derive(FromArgs)]
#[argh(subcommand, name = "check")]
/// Check handlist syntax, optionally validating against a JSON Schema.
struct CheckArgs {
    /// JSON Schema file (draft-07 or 2020-12)
    #[argh(option)]
    schema: Option<PathBuf>,
    /// also recurse into .git, target, and node_modules
    #[argh(switch, short = 'a')]
    all: bool,
    /// files or directories to check
    #[argh(positional)]
    paths: Vec<PathBuf>,
}

#[derive(FromArgs)]
#[argh(subcommand, name = "fmt")]
/// Format handlist files in place.
struct FmtArgs {
    /// check formatting without writing files
    #[argh(switch)]
    check: bool,
    /// also recurse into .git, target, and node_modules
    #[argh(switch, short = 'a')]
    all: bool,
    /// files or directories to format
    #[argh(positional)]
    paths: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let cli: Handlist = argh::from_env();
    match cli.command {
        Command::Check(args) => check(args),
        Command::Fmt(args) => fmt(args),
    }
}

#[derive(Default)]
struct CheckStats {
    files: usize,
    errors: usize,
}

fn check(args: CheckArgs) -> ExitCode {
    let paths = default_paths(args.paths);
    let schema = match args.schema {
        Some(path) => match load_schema(&path) {
            Ok(schema) => Some(schema),
            Err(()) => return ExitCode::FAILURE,
        },
        None => None,
    };

    let (files, mut stats) = collect_files(&paths, args.all);
    if files.is_empty() && stats.errors == 0 {
        eprintln!("handlist: no .handlist files found");
        return ExitCode::SUCCESS;
    }

    for path in files {
        stats.files += 1;
        let Some(source) = read_source(&path, &mut stats.errors) else {
            continue;
        };

        if let Some(schema) = &schema {
            match handlist::validate_json_schema(&source, schema) {
                Ok(result) if result.is_valid() => {}
                Ok(result) => {
                    for error in result.errors() {
                        eprintln!("{}: {error}", path.display());
                    }
                    stats.errors += 1;
                }
                Err(error) => report_handlist_error(&path, &error, &mut stats.errors),
            }
        } else if let Err(error) = handlist::parse(&source) {
            report_handlist_error(&path, &error, &mut stats.errors);
        }
    }

    if stats.errors == 0 {
        if schema.is_some() {
            println!("checked {} file(s), valid against schema", stats.files);
        } else {
            println!("checked {} file(s), no syntax errors", stats.files);
        }
        ExitCode::SUCCESS
    } else {
        println!("checked {} file(s), {} error(s)", stats.files, stats.errors);
        ExitCode::FAILURE
    }
}

#[derive(Default)]
struct FmtStats {
    files: usize,
    changed: usize,
    errors: usize,
}

fn fmt(args: FmtArgs) -> ExitCode {
    let paths = default_paths(args.paths);
    let (files, mut stats) = collect_fmt_files(&paths, args.all);
    if files.is_empty() && stats.errors == 0 {
        eprintln!("handlist: no .handlist files found");
        return ExitCode::SUCCESS;
    }

    for path in files {
        stats.files += 1;
        let Some(source) = read_source(&path, &mut stats.errors) else {
            continue;
        };
        let formatted = match handlist::format(&source) {
            Ok(formatted) => formatted,
            Err(error) => {
                report_handlist_error(&path, &error, &mut stats.errors);
                continue;
            }
        };

        if formatted == source {
            continue;
        }
        stats.changed += 1;

        if args.check {
            eprintln!("{}: not formatted", path.display());
            continue;
        }
        if let Err(error) = fs::write(&path, formatted) {
            eprintln!("{}: error: cannot write file: {error}", path.display());
            stats.errors += 1;
        }
    }

    let status = if args.check {
        println!(
            "checked {} file(s), {} need formatting",
            stats.files, stats.changed
        );
        if stats.changed == 0 && stats.errors == 0 {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    } else {
        println!(
            "formatted {} file(s), {} changed",
            stats.files, stats.changed
        );
        if stats.errors == 0 {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    };

    if stats.errors > 0 {
        eprintln!("{} error(s)", stats.errors);
    }
    status
}

fn default_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths
    }
}

fn load_schema(path: &Path) -> Result<String, ()> {
    match fs::read_to_string(path) {
        Ok(source) => {
            if let Err(error) = handlist::validate_json_schema("null\n", &source) {
                eprintln!("{}: error: {}", path.display(), error.message());
                return Err(());
            }
            Ok(source)
        }
        Err(error) => {
            eprintln!("{}: error: cannot read schema: {error}", path.display());
            Err(())
        }
    }
}

fn read_source(path: &Path, errors: &mut usize) -> Option<String> {
    match fs::read(path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(source) => Some(source),
            Err(error) => {
                eprintln!(
                    "{}: error: file is not valid UTF-8 ({error})",
                    path.display()
                );
                *errors += 1;
                None
            }
        },
        Err(error) => {
            eprintln!("{}: error: cannot read file: {error}", path.display());
            *errors += 1;
            None
        }
    }
}

fn report_handlist_error(path: &Path, error: &handlist::Error, errors: &mut usize) {
    *errors += 1;
    match (error.line(), error.column()) {
        (Some(line), Some(column)) => {
            eprintln!(
                "{}:{line}:{column}: error: {}",
                path.display(),
                error.message()
            );
        }
        _ => {
            eprintln!("{}: error: {}", path.display(), error.message());
        }
    }
}

fn collect_files(paths: &[PathBuf], all: bool) -> (Vec<PathBuf>, CheckStats) {
    let mut files = Vec::new();
    let mut stats = CheckStats::default();
    for path in paths {
        collect_path(path, all, &mut files, &mut stats.errors);
    }
    (files, stats)
}

fn collect_fmt_files(paths: &[PathBuf], all: bool) -> (Vec<PathBuf>, FmtStats) {
    let mut files = Vec::new();
    let mut stats = FmtStats::default();
    for path in paths {
        collect_path(path, all, &mut files, &mut stats.errors);
    }
    (files, stats)
}

fn collect_path(path: &Path, all: bool, files: &mut Vec<PathBuf>, errors: &mut usize) {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => files.push(path.to_path_buf()),
        Ok(metadata) if metadata.is_dir() => collect_directory(path, all, files, errors),
        Ok(_) => {
            eprintln!("{}: error: not a regular file or directory", path.display());
            *errors += 1;
        }
        Err(error) => {
            eprintln!("{}: error: cannot access path: {error}", path.display());
            *errors += 1;
        }
    }
}

fn collect_directory(directory: &Path, all: bool, files: &mut Vec<PathBuf>, errors: &mut usize) {
    let mut entries = Vec::new();
    match fs::read_dir(directory) {
        Ok(read_dir) => {
            for entry in read_dir {
                match entry {
                    Ok(entry) => entries.push(entry),
                    Err(error) => {
                        eprintln!(
                            "{}: error: cannot read directory entry: {error}",
                            directory.display()
                        );
                        *errors += 1;
                    }
                }
            }
        }
        Err(error) => {
            eprintln!(
                "{}: error: cannot read directory: {error}",
                directory.display()
            );
            *errors += 1;
            return;
        }
    }
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                eprintln!("{}: error: cannot inspect path: {error}", path.display());
                *errors += 1;
                continue;
            }
        };
        let is_symlink = file_type.is_symlink();
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                if !is_symlink {
                    eprintln!("{}: error: cannot inspect path: {error}", path.display());
                    *errors += 1;
                }
                continue;
            }
        };

        if metadata.is_dir() {
            if is_symlink {
                continue;
            }
            if !all && is_skipped_directory(&entry.file_name()) {
                continue;
            }
            collect_directory(&path, all, files, errors);
        } else if metadata.is_file() && is_handlist_file(&path) {
            files.push(path);
        }
    }
}

fn is_skipped_directory(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| SKIPPED_DIRECTORIES.contains(&name))
}

fn is_handlist_file(path: &Path) -> bool {
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|name| name.ends_with(".handlist"))
}
