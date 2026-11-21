#![cfg(feature = "format")]

use std::fs;
use std::path::{Path, PathBuf};

fn examples_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples")
}

fn handlist_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("examples directory is readable") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            handlist_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "handlist") {
            out.push(path);
        }
    }
}

#[test]
fn canonical_fixtures_are_unchanged() {
    let root = examples_root();
    if !root.exists() {
        eprintln!(
            "skipping example fixtures: {} does not exist",
            root.display()
        );
        return;
    }

    let canonical_root = root.join("canonical");
    let mut files = Vec::new();
    handlist_files(&canonical_root, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no canonical fixtures found");

    for path in files {
        let source = fs::read_to_string(&path).expect("fixture is readable");
        let formatted =
            handlist::format(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert_eq!(
            formatted,
            source,
            "canonical fixture changed: {}",
            path.display()
        );
    }
}

#[test]
fn preserves_full_line_and_inline_comments() {
    let source = "\
# top

site:
  name: \"Example\"   # inline
  pages:
    - name: home
      title: \"Home\"
  # trailing site
other: \"value\"
# tail
";
    let expected = "\
# top

site:
  name: Example # inline
  pages:
    - name: home
      title: Home
  # trailing site
other: value
# tail
";
    assert_eq!(handlist::format(source).expect("valid source"), expected);
}

#[test]
fn preserves_commented_out_blocks() {
    let source = "\
list:
  - name: one
    # - name: two
    #   title: Two
  - name: three
";
    assert_eq!(handlist::format(source).expect("valid source"), source);
}

#[test]
fn preserves_comments_when_flow_sequences_expand() {
    let source = "\
tags: [a, b] # tags
items:
  - [x, y] # item
";
    let expected = "\
tags:
  - a
  - b
  # tags
items:
  - - x
    - y
  # item
";
    assert_eq!(handlist::format(source).expect("valid source"), expected);
    assert_eq!(
        handlist::format(expected).expect("formatted output is valid"),
        expected
    );
}

#[test]
fn preserves_block_text_header_comments_and_hash_content() {
    let source = "\
text: | # header comment
  line one
  line two
inline: | # becomes inline
  one line
hash: |
  # not a comment
";
    let expected = "\
text: | # header comment
  line one
  line two
inline: one line # becomes inline
hash: \"# not a comment\"
";
    assert_eq!(handlist::format(source).expect("valid source"), expected);
}

#[test]
fn normalizes_blank_lines_around_comments_and_top_level_nodes() {
    let source = "\
# top


site:
  name: x

  # nested comment
  other: y


root2: z
";
    let expected = "\
# top

site:
  name: x

  # nested comment
  other: y

root2: z
";
    assert_eq!(handlist::format(source).expect("valid source"), expected);
}

#[test]
fn is_idempotent_on_commented_examples() {
    let root = examples_root();
    if !root.exists() {
        eprintln!(
            "skipping example fixtures: {} does not exist",
            root.display()
        );
        return;
    }

    let mut files = Vec::new();
    handlist_files(&root, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no example files found");

    for path in files {
        let source = fs::read_to_string(&path).expect("example is readable");
        let once =
            handlist::format(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let twice = handlist::format(&once)
            .unwrap_or_else(|error| panic!("{} (formatted): {error}", path.display()));
        assert_eq!(
            twice,
            once,
            "formatter is not idempotent for {}",
            path.display()
        );
    }
}
