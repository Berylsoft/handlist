use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_handlist-check")
}

fn run(path: &Path) -> Output {
    Command::new(binary())
        .arg(path)
        .output()
        .expect("run handlist-check")
}

fn run_with_all(path: &Path) -> Output {
    Command::new(binary())
        .arg("--all")
        .arg(path)
        .output()
        .expect("run handlist-check --all")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn exit_code(output: &Output) -> i32 {
    output
        .status
        .code()
        .expect("handlist-check exits with a code")
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

    fn write(&self, relative: &str, contents: &str) {
        self.write_bytes(relative, contents.as_bytes());
    }

    fn write_bytes(&self, relative: &str, contents: &[u8]) {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(path, contents).expect("write test file");
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn reports_nested_syntax_errors_with_line_numbers() {
    let tree = TempTree::new("nested");
    tree.write("good.handlist", "name: ok\n");
    tree.write("nested/bad.handlist", "a: 1\nb: [x,,y]\n");

    let output = run(tree.path());
    let err = stderr(&output);

    assert_eq!(exit_code(&output), 1);
    assert!(
        err.contains("bad.handlist:2:"),
        "expected a line-2 diagnostic, got:\n{err}"
    );
    assert!(
        err.contains("empty flow-sequence element"),
        "expected the parser diagnostic, got:\n{err}"
    );
    assert!(
        stdout(&output).contains("checked 2 file(s), 1 error(s)"),
        "unexpected summary:\n{}",
        stdout(&output)
    );
}

#[test]
fn checks_explicit_files_without_handlist_extension() {
    let tree = TempTree::new("explicit");
    tree.write("config.txt", "name: ok\n");

    let output = Command::new(binary())
        .arg(tree.path().join("config.txt"))
        .output()
        .expect("run handlist-check on an explicit file");

    assert_eq!(exit_code(&output), 0);
    assert!(stdout(&output).contains("checked 1 file(s), no syntax errors"));
}

#[test]
fn clean_tree_succeeds() {
    let tree = TempTree::new("clean");
    tree.write("good.handlist", "name: ok\n");

    let output = run(tree.path());

    assert_eq!(exit_code(&output), 0);
    assert!(stdout(&output).contains("checked 1 file(s), no syntax errors"));
    assert!(stderr(&output).is_empty());
}

#[test]
fn empty_directory_succeeds_with_a_notice() {
    let tree = TempTree::new("empty");

    let output = run(tree.path());

    assert_eq!(exit_code(&output), 0);
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("no .handlist files found"));
}

#[test]
fn missing_path_is_an_error() {
    let tree = TempTree::new("missing");
    let missing = tree.path().join("does-not-exist");

    let output = run(&missing);

    assert_eq!(exit_code(&output), 1);
    assert!(stderr(&output).contains("cannot access path"));
    assert!(stdout(&output).contains("checked 0 file(s), 1 error(s)"));
}

#[test]
fn skips_build_directories_unless_all_is_used() {
    let tree = TempTree::new("skipped");
    tree.write("good.handlist", "name: ok\n");
    tree.write("target/bad.handlist", "a: 1\nb: [x,,y]\n");

    let default_output = run(tree.path());
    assert_eq!(exit_code(&default_output), 0);
    assert!(stdout(&default_output).contains("checked 1 file(s), no syntax errors"));

    let all_output = run_with_all(tree.path());
    let err = stderr(&all_output);
    assert_eq!(exit_code(&all_output), 1);
    assert!(
        err.contains("bad.handlist:2:"),
        "expected the bad file under target/, got:\n{err}"
    );
    assert!(stdout(&all_output).contains("checked 2 file(s), 1 error(s)"));
}

#[test]
fn reports_files_that_are_not_valid_utf8() {
    let tree = TempTree::new("utf8");
    tree.write_bytes("bad.handlist", &[0xff, 0xfe]);

    let output = run(tree.path());

    assert_eq!(exit_code(&output), 1);
    assert!(stderr(&output).contains("not valid UTF-8"));
    assert!(stdout(&output).contains("checked 1 file(s), 1 error(s)"));
}

#[test]
fn help_version_and_usage_errors() {
    let help = Command::new(binary())
        .arg("--help")
        .output()
        .expect("run handlist-check --help");
    assert_eq!(exit_code(&help), 0);
    let help_text = stdout(&help);
    assert!(help_text.contains("Usage:"));
    assert!(help_text.contains("handlist-check"));

    let version = Command::new(binary())
        .arg("--version")
        .output()
        .expect("run handlist-check --version");
    assert_eq!(exit_code(&version), 0);
    assert!(stdout(&version).contains(env!("CARGO_PKG_VERSION")));

    let unknown = Command::new(binary())
        .arg("--not-an-option")
        .output()
        .expect("run handlist-check with an unknown option");
    assert_eq!(exit_code(&unknown), 2);
    assert!(stderr(&unknown).contains("unknown option"));
}
