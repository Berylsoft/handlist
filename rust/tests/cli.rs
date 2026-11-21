#![cfg(feature = "cli")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_handlist")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn exit_code(output: &Output) -> i32 {
    output.status.code().expect("handlist exits with a code")
}

struct TempTree {
    path: PathBuf,
}

impl TempTree {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after the epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "handlist-cli-{}-{label}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary tree");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(&path, contents).expect("write test file");
        path
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.path.join(relative)).expect("read test file")
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn check_path(path: &Path) -> Output {
    Command::new(binary())
        .arg("check")
        .arg(path)
        .output()
        .expect("run handlist check")
}

#[test]
fn check_reports_syntax_errors_recursively() {
    let tree = TempTree::new("syntax");
    tree.write("good.handlist", "name: ok\n");
    tree.write("nested/bad.handlist", "a: 1\nb: [x,,y]\n");

    let output = check_path(tree.path());
    let err = stderr(&output);

    assert_eq!(exit_code(&output), 1);
    assert!(
        err.contains("bad.handlist:2:"),
        "expected a line-2 diagnostic, got:\n{err}"
    );
    assert!(
        err.contains("empty flow-sequence element"),
        "expected parser diagnostics, got:\n{err}"
    );
    assert!(stdout(&output).contains("checked 2 file(s), 1 error(s)"));
}

#[test]
fn check_skips_build_directories_unless_all_is_used() {
    let tree = TempTree::new("skipped");
    tree.write("good.handlist", "name: ok\n");
    tree.write("target/bad.handlist", "a: 1\nb: [x,,y]\n");

    let default_output = check_path(tree.path());
    assert_eq!(exit_code(&default_output), 0);
    assert!(stdout(&default_output).contains("checked 1 file(s), no syntax errors"));

    let all_output = Command::new(binary())
        .arg("check")
        .arg(tree.path())
        .arg("--all")
        .output()
        .expect("run handlist check --all");
    assert_eq!(exit_code(&all_output), 1);
    assert!(stderr(&all_output).contains("bad.handlist:2:"));
    assert!(stdout(&all_output).contains("checked 2 file(s), 1 error(s)"));
}

#[test]
fn check_validates_against_a_schema_with_option_after_positional() {
    let tree = TempTree::new("schema");
    let schema = tree.write(
        "schema.json",
        r#"{
  "type": "object",
  "required": ["name", "count"],
  "properties": {
    "name": { "type": "string" },
    "count": { "type": "integer" }
  },
  "additionalProperties": false
}"#,
    );
    let valid = tree.write("valid.handlist", "name: Example\ncount: 2\n");
    let invalid = tree.write("invalid.handlist", "name: Example\ncount: nope\n");

    let valid_output = Command::new(binary())
        .arg("check")
        .arg(&valid)
        .arg("--schema")
        .arg(&schema)
        .output()
        .expect("run valid schema check");
    assert_eq!(exit_code(&valid_output), 0);
    assert!(stdout(&valid_output).contains("valid against schema"));

    let invalid_output = Command::new(binary())
        .arg("check")
        .arg(&invalid)
        .arg("--schema")
        .arg(&schema)
        .output()
        .expect("run invalid schema check");
    assert_eq!(exit_code(&invalid_output), 1);
    assert!(
        stderr(&invalid_output).contains("/count"),
        "expected instance path, got:\n{}",
        stderr(&invalid_output)
    );
    assert!(stderr(&invalid_output).contains("not of type \"integer\""));
}

#[test]
fn check_reports_invalid_schema_files() {
    let tree = TempTree::new("bad-schema");
    let schema = tree.write("schema.json", "{");
    let file = tree.write("file.handlist", "name: x\n");

    let output = Command::new(binary())
        .arg("check")
        .arg(&file)
        .arg("--schema")
        .arg(&schema)
        .output()
        .expect("run handlist check with an invalid schema");
    assert_eq!(exit_code(&output), 1);
    assert!(stderr(&output).contains("invalid JSON Schema"));
}

#[test]
fn fmt_writes_in_place_and_preserves_comments() {
    let tree = TempTree::new("fmt");
    let file = tree.write("file.handlist", "# top\nname: \"Example\"   # inline\n");

    let output = Command::new(binary())
        .arg("fmt")
        .arg(&file)
        .output()
        .expect("run handlist fmt");
    assert_eq!(exit_code(&output), 0);
    assert_eq!(
        tree.read("file.handlist"),
        "# top\nname: Example # inline\n"
    );

    let checked = Command::new(binary())
        .arg("fmt")
        .arg(&file)
        .arg("--check")
        .output()
        .expect("run handlist fmt --check");
    assert_eq!(exit_code(&checked), 0);
}

#[test]
fn fmt_check_reports_without_writing() {
    let tree = TempTree::new("fmt-check");
    let file = tree.write("file.handlist", "name: \"Example\"\n");
    let before = tree.read("file.handlist");

    let output = Command::new(binary())
        .arg("fmt")
        .arg(&file)
        .arg("--check")
        .output()
        .expect("run handlist fmt --check");
    assert_eq!(exit_code(&output), 1);
    assert!(stderr(&output).contains("not formatted"));
    assert_eq!(tree.read("file.handlist"), before);
}

#[test]
fn fmt_recurses_and_skips_build_directories_by_default() {
    let tree = TempTree::new("fmt-dir");
    tree.write("a.handlist", "name: \"A\"\n");
    tree.write("target/b.handlist", "name: \"B\"\n");

    let output = Command::new(binary())
        .arg("fmt")
        .arg(tree.path())
        .output()
        .expect("run handlist fmt on a directory");
    assert_eq!(exit_code(&output), 0);
    assert_eq!(tree.read("a.handlist"), "name: A\n");
    assert_eq!(tree.read("target/b.handlist"), "name: \"B\"\n");

    let all_output = Command::new(binary())
        .arg("fmt")
        .arg(tree.path())
        .arg("--all")
        .output()
        .expect("run handlist fmt --all on a directory");
    assert_eq!(exit_code(&all_output), 0);
    assert_eq!(tree.read("target/b.handlist"), "name: B\n");
}

#[test]
fn fmt_without_paths_uses_the_current_directory() {
    let tree = TempTree::new("fmt-cwd");
    tree.write("file.handlist", "name: \"Example\"\n");

    let output = Command::new(binary())
        .arg("fmt")
        .current_dir(tree.path())
        .output()
        .expect("run handlist fmt in the current directory");
    assert_eq!(exit_code(&output), 0);
    assert_eq!(tree.read("file.handlist"), "name: Example\n");
}

#[test]
fn fmt_uses_fmt_handlist_from_the_current_directory() {
    let tree = TempTree::new("fmt-config");
    tree.write(
        ".fmt.handlist",
        "version: 1\nallow:\n  - non-complex-one-line-seq\n  - one-line-in-multi-line-text\n",
    );
    tree.write("file.handlist", "tags: [a, b]\ntext: |\n  one line\n");

    let output = Command::new(binary())
        .arg("fmt")
        .arg(tree.path().join("file.handlist"))
        .current_dir(tree.path())
        .output()
        .expect("run handlist fmt with .fmt.handlist");
    assert_eq!(exit_code(&output), 0);
    assert_eq!(
        tree.read("file.handlist"),
        "tags: [a, b]\ntext: |\n  one line\n"
    );
}

#[test]
fn fmt_config_option_overrides_the_current_directory() {
    let tree = TempTree::new("fmt-config-override");
    let config = tree.write(
        "style.handlist",
        "version: 1\nallow:\n  - non-complex-one-line-seq\n",
    );
    let file = tree.write("file.handlist", "tags: [a, b]\n");

    let output = Command::new(binary())
        .arg("fmt")
        .arg(&file)
        .arg("--config")
        .arg(&config)
        .current_dir(tree.path())
        .output()
        .expect("run handlist fmt --config");
    assert_eq!(exit_code(&output), 0);
    assert_eq!(tree.read("file.handlist"), "tags: [a, b]\n");
}

#[test]
fn fmt_reports_invalid_fmt_handlist() {
    let tree = TempTree::new("fmt-bad-config");
    tree.write(".fmt.handlist", "version: 2\n");
    let file = tree.write("file.handlist", "name: \"Example\"\n");
    let before = tree.read("file.handlist");

    let output = Command::new(binary())
        .arg("fmt")
        .arg(&file)
        .current_dir(tree.path())
        .output()
        .expect("run handlist fmt with an invalid config");
    assert_eq!(exit_code(&output), 1);
    assert!(stderr(&output).contains("unsupported format config version 2"));
    assert_eq!(tree.read("file.handlist"), before);
}

#[test]
fn empty_directories_are_reported_but_do_not_fail() {
    let tree = TempTree::new("empty");
    let output = check_path(tree.path());
    assert_eq!(exit_code(&output), 0);
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("no .handlist files found"));
}
