use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn examples_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples")
}

fn example_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("examples directory is readable") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "canonical") {
                continue;
            }
            example_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "handlist") {
            out.push(path);
        }
    }
}

fn canonical_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("canonical directory is readable") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            canonical_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "handlist") {
            out.push(path);
        }
    }
}

#[test]
fn examples_match_their_canonical_fixtures() {
    let root = examples_root();
    if !root.exists() {
        eprintln!(
            "skipping example fixtures: {} does not exist",
            root.display()
        );
        return;
    }
    let canonical_root = root.join("canonical");

    let mut sources = Vec::new();
    example_files(&root, &mut sources);
    sources.sort();
    assert!(!sources.is_empty(), "no example files found");

    let mut values = BTreeMap::new();

    for path in &sources {
        let relative = path.strip_prefix(&root).expect("example path");
        let source = fs::read_to_string(path).expect("example is readable");
        let value =
            handlist::parse(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let canonical = handlist::to_string_value(&value)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));

        let fixture_path = canonical_root.join(relative);
        let fixture = fs::read_to_string(&fixture_path).unwrap_or_else(|error| {
            panic!(
                "missing canonical fixture {}: {error}",
                fixture_path.display()
            )
        });
        assert_eq!(
            canonical,
            fixture,
            "canonical output differs from fixture for {}",
            path.display()
        );

        let reparsed = handlist::parse(&canonical).unwrap_or_else(|error| {
            panic!(
                "{} (canonical output): {error}\n{canonical}",
                path.display()
            )
        });
        assert_eq!(
            value,
            reparsed,
            "round-trip mismatch for {}",
            path.display()
        );

        values.insert(relative.to_path_buf(), value);
    }

    let mut fixtures = Vec::new();
    canonical_files(&canonical_root, &mut fixtures);
    fixtures.sort();
    assert_eq!(fixtures.len(), sources.len(), "stale canonical fixtures");

    for path in fixtures {
        let relative = path
            .strip_prefix(&canonical_root)
            .expect("canonical fixture path");
        let text = fs::read_to_string(&path).expect("canonical fixture is readable");
        let value =
            handlist::parse(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(
            values.get(relative),
            Some(&value),
            "canonical fixture has a different value: {}",
            path.display()
        );
        assert_eq!(
            handlist::to_string_value(&value).expect("canonical output"),
            text,
            "canonical fixture is not idempotent: {}",
            path.display()
        );
    }
}
