//! Prints the canonical handlist form of one or more files.
//!
//! Usage: `cargo run --example canonicalize -- path/to/file.handlist ...`.
//! This example is also used to regenerate the canonical fixtures in
//! `examples/canonical/`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let mut files: Vec<PathBuf> = env::args().skip(1).map(PathBuf::from).collect();
    if files.is_empty() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples");
        if !root.exists() {
            eprintln!("examples directory not found: {}", root.display());
            return;
        }
        collect(&root, &mut files);
        files.sort();
        let canonical_root = root.join("canonical");
        for path in files {
            let relative = path.strip_prefix(&root).expect("example path");
            let source = fs::read_to_string(&path).expect("example is readable");
            let value = handlist::parse(&source)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let text = handlist::to_string_value(&value)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let destination = canonical_root.join(relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).expect("create canonical directory");
            }
            fs::write(&destination, text).expect("write canonical file");
        }
    } else {
        for path in files.drain(..) {
            let source = fs::read_to_string(&path).expect("file is readable");
            let value = handlist::parse(&source)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            print!(
                "{}",
                handlist::to_string_value(&value)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
            );
        }
    }
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("examples directory is readable") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "canonical") {
                continue;
            }
            collect(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "handlist") {
            files.push(path);
        }
    }
}
