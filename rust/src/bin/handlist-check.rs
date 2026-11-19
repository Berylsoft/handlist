//! `handlist-check`: recursively check handlist syntax in files and directories.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Directories that are skipped during recursive traversal unless `--all` is
/// given. They are common build/VCS directories and rarely contain handlist
/// source files.
const SKIPPED_DIRECTORIES: &[&str] = &[".git", "target", "node_modules"];

fn main() -> ExitCode {
    let mut paths = Vec::new();
    let mut all = false;
    let mut options_done = false;

    for arg in env::args_os().skip(1) {
        if !options_done {
            match arg.to_str() {
                Some("--") => {
                    options_done = true;
                    continue;
                }
                Some("-h") | Some("--help") => {
                    print_help();
                    return ExitCode::SUCCESS;
                }
                Some("-V") | Some("--version") => {
                    println!("handlist-check {VERSION}");
                    return ExitCode::SUCCESS;
                }
                Some("-a") | Some("--all") => {
                    all = true;
                    continue;
                }
                Some(value) if value.starts_with('-') && value != "-" => {
                    eprintln!("handlist-check: unknown option '{value}'");
                    eprintln!("Try 'handlist-check --help' for more information.");
                    return ExitCode::from(2);
                }
                _ => {}
            }
        }
        paths.push(PathBuf::from(arg));
    }

    if paths.is_empty() {
        paths.push(PathBuf::from("."));
    }

    let mut stats = Stats::default();
    for path in &paths {
        check_path(path, all, &mut stats);
    }

    if stats.files == 0 && stats.errors == 0 {
        eprintln!("handlist-check: no .handlist files found");
        return ExitCode::SUCCESS;
    }

    if stats.errors == 0 {
        println!("checked {} file(s), no syntax errors", stats.files);
        ExitCode::SUCCESS
    } else {
        println!("checked {} file(s), {} error(s)", stats.files, stats.errors);
        ExitCode::FAILURE
    }
}

#[derive(Default)]
struct Stats {
    files: usize,
    errors: usize,
}

fn check_path(path: &Path, all: bool, stats: &mut Stats) {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => check_file(path, stats),
        Ok(metadata) if metadata.is_dir() => check_directory(path, all, stats),
        Ok(_) => {
            eprintln!("{}: error: not a regular file or directory", path.display());
            stats.errors += 1;
        }
        Err(error) => {
            eprintln!("{}: error: cannot access path: {error}", path.display());
            stats.errors += 1;
        }
    }
}

fn check_directory(directory: &Path, all: bool, stats: &mut Stats) {
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
                        stats.errors += 1;
                    }
                }
            }
        }
        Err(error) => {
            eprintln!(
                "{}: error: cannot read directory: {error}",
                directory.display()
            );
            stats.errors += 1;
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
                stats.errors += 1;
                continue;
            }
        };
        let is_symlink = file_type.is_symlink();

        // Follow symlinked files, but never follow symlinked directories.
        // That keeps directory traversal free of symlink loops.
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                if !is_symlink {
                    eprintln!("{}: error: cannot inspect path: {error}", path.display());
                    stats.errors += 1;
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
            check_directory(&path, all, stats);
        } else if metadata.is_file() && is_handlist_file(&path) {
            check_file(&path, stats);
        }
    }
}

fn check_file(path: &Path, stats: &mut Stats) {
    stats.files += 1;

    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("{}: error: cannot read file: {error}", path.display());
            stats.errors += 1;
            return;
        }
    };
    let source = match String::from_utf8(bytes) {
        Ok(source) => source,
        Err(error) => {
            eprintln!(
                "{}: error: file is not valid UTF-8 ({error})",
                path.display()
            );
            stats.errors += 1;
            return;
        }
    };

    if let Err(error) = handlist::parse(&source) {
        stats.errors += 1;
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
}

fn is_skipped_directory(name: &OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| SKIPPED_DIRECTORIES.contains(&name))
}

fn is_handlist_file(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.ends_with(".handlist"))
}

fn print_help() {
    println!("handlist-check {VERSION}");
    println!();
    println!("Check handlist syntax in files and directories.");
    println!();
    println!("Usage:");
    println!("  handlist-check [OPTIONS] [PATH]...");
    println!();
    println!("Arguments:");
    println!("  PATH...  Explicit files are checked regardless of extension.");
    println!("           Directories are searched recursively for files whose names");
    println!("           end with \".handlist\". With no PATH, \".\" is checked.");
    println!();
    println!("Options:");
    println!("  -a, --all       Also recurse into .git, target, and node_modules");
    println!("  -h, --help      Print this help");
    println!("  -V, --version   Print version");
    println!();
    println!("Diagnostics:");
    println!("  Syntax errors: path:line:column: error: message");
    println!("  I/O errors:    path: error: message");
    println!("  The first syntax error in each file is reported.");
    println!("  Directory symlinks are not followed while recursing.");
    println!();
    println!("Exit status: 0 = no errors, 1 = errors found, 2 = usage error.");
}
