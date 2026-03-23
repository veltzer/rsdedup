use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;

fn rsdedup_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rsdedup"))
}

fn create_test_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn write_file(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, content).unwrap();
}

#[test]
fn report_no_duplicates() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "hello");
    write_file(dir.path(), "b.txt", "world");

    let output = rsdedup_bin()
        .args(["report", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success(), "expected exit code 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 0"));
}

#[test]
fn report_finds_duplicates() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "duplicate content");
    write_file(dir.path(), "b.txt", "duplicate content");
    write_file(dir.path(), "c.txt", "unique content");

    let output = rsdedup_bin()
        .args(["report", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1), "expected exit code 1 (dupes found)");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
    assert!(stdout.contains("Duplicate files:  1"));
    assert!(stdout.contains("a.txt"));
    assert!(stdout.contains("b.txt"));
}

#[test]
fn report_json_output() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "same");
    write_file(dir.path(), "b.txt", "same");

    let output = rsdedup_bin()
        .args(["report", "--no-cache", "--output", "json"])
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"group\""));
    assert!(stdout.contains("\"size\""));
    assert!(stdout.contains("\"files\""));
}

#[test]
fn delete_dry_run_does_not_remove_files() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "dup");
    write_file(dir.path(), "b.txt", "dup");

    let output = rsdedup_bin()
        .args(["delete", "--no-cache", "--dry-run"])
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("would delete"));
    assert!(dir.path().join("a.txt").exists());
    assert!(dir.path().join("b.txt").exists());
}

#[test]
fn delete_removes_duplicates() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "dup content here");
    write_file(dir.path(), "b.txt", "dup content here");
    write_file(dir.path(), "unique.txt", "not a dup");

    let output = rsdedup_bin()
        .args(["delete", "--no-cache", "--keep", "first"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Files affected:   1"));
    assert!(dir.path().join("unique.txt").exists());
    // One of the two should remain, one should be deleted
    let a_exists = dir.path().join("a.txt").exists();
    let b_exists = dir.path().join("b.txt").exists();
    assert!(a_exists || b_exists, "at least one copy should remain");
    assert!(!(a_exists && b_exists), "one copy should have been deleted");
}

#[test]
fn hardlink_dry_run() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "hardlink me");
    write_file(dir.path(), "b.txt", "hardlink me");

    let output = rsdedup_bin()
        .args(["hardlink", "--no-cache", "--dry-run"])
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("would hardlink"));
    // Both files should still be separate
    let ino_a = fs::metadata(dir.path().join("a.txt")).unwrap().ino();
    let ino_b = fs::metadata(dir.path().join("b.txt")).unwrap().ino();
    assert_ne!(ino_a, ino_b);
}

#[test]
fn hardlink_creates_hardlinks() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "link this content");
    write_file(dir.path(), "b.txt", "link this content");

    rsdedup_bin()
        .args(["hardlink", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    let ino_a = fs::metadata(dir.path().join("a.txt")).unwrap().ino();
    let ino_b = fs::metadata(dir.path().join("b.txt")).unwrap().ino();
    assert_eq!(ino_a, ino_b, "files should share the same inode after hardlinking");
}

#[test]
fn symlink_creates_symlinks() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "symlink this");
    write_file(dir.path(), "b.txt", "symlink this");

    rsdedup_bin()
        .args(["symlink", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    let a_is_symlink = fs::symlink_metadata(dir.path().join("a.txt"))
        .unwrap()
        .file_type()
        .is_symlink();
    let b_is_symlink = fs::symlink_metadata(dir.path().join("b.txt"))
        .unwrap()
        .file_type()
        .is_symlink();
    assert!(a_is_symlink || b_is_symlink, "at least one file should be a symlink");
}

#[test]
fn scan_populates_cache() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "cache me");

    let output = rsdedup_bin()
        .args(["scan"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("scanned and cached"));
}

#[test]
fn min_size_filter() {
    let dir = create_test_dir();
    write_file(dir.path(), "small1.txt", "ab");
    write_file(dir.path(), "small2.txt", "ab");
    write_file(dir.path(), "big1.txt", "this is bigger content");
    write_file(dir.path(), "big2.txt", "this is bigger content");

    let output = rsdedup_bin()
        .args(["report", "--no-cache", "--min-size", "10"])
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
    assert!(!stdout.contains("small1.txt"));
}

#[test]
fn max_size_filter() {
    let dir = create_test_dir();
    write_file(dir.path(), "small1.txt", "ab");
    write_file(dir.path(), "small2.txt", "ab");
    write_file(dir.path(), "big1.txt", "this is bigger content");
    write_file(dir.path(), "big2.txt", "this is bigger content");

    let output = rsdedup_bin()
        .args(["report", "--no-cache", "--max-size", "10"])
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
    assert!(!stdout.contains("big1.txt"));
}

#[test]
fn exclude_filter() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "dup");
    write_file(dir.path(), "b.txt", "dup");
    write_file(dir.path(), "a.log", "dup");
    write_file(dir.path(), "b.log", "dup");

    let output = rsdedup_bin()
        .args(["report", "--no-cache", "--exclude", "*.log"])
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("a.txt"));
    assert!(!stdout.contains(".log"));
}

#[test]
fn include_filter() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "dup");
    write_file(dir.path(), "b.txt", "dup");
    write_file(dir.path(), "a.log", "dup");
    write_file(dir.path(), "b.log", "dup");

    let output = rsdedup_bin()
        .args(["report", "--no-cache", "--include", "*.log"])
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains(".txt"));
    assert!(stdout.contains(".log"));
}

#[test]
fn compare_byte_for_byte() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "same bytes");
    write_file(dir.path(), "b.txt", "same bytes");

    let output = rsdedup_bin()
        .args(["report", "--no-cache", "--compare", "byte-for-byte"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
}

#[test]
fn compare_hash_only() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "hash me");
    write_file(dir.path(), "b.txt", "hash me");

    let output = rsdedup_bin()
        .args(["report", "--no-cache", "--compare", "hash"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
}

#[test]
fn hash_algo_blake3() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "blake3 test");
    write_file(dir.path(), "b.txt", "blake3 test");

    let output = rsdedup_bin()
        .args(["report", "--no-cache", "--hash", "blake3"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn hash_algo_xxhash() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "xxhash test");
    write_file(dir.path(), "b.txt", "xxhash test");

    let output = rsdedup_bin()
        .args(["report", "--no-cache", "--hash", "xxhash"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn version_subcommand() {
    let output = rsdedup_bin().args(["version"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rsdedup"));
    assert!(stdout.contains("GIT_SHA"));
    assert!(stdout.contains("BUILD_TIMESTAMP"));
}

#[test]
fn completions_subcommand() {
    let output = rsdedup_bin()
        .args(["completions", "bash"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rsdedup"));
}

#[test]
fn subdirectory_duplicates() {
    let dir = create_test_dir();
    write_file(dir.path(), "a/file.txt", "nested dup");
    write_file(dir.path(), "b/file.txt", "nested dup");

    let output = rsdedup_bin()
        .args(["report", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
}

#[test]
fn no_recursive_flag() {
    let dir = create_test_dir();
    write_file(dir.path(), "top.txt", "dup");
    write_file(dir.path(), "sub/nested.txt", "dup");

    let output = rsdedup_bin()
        .args(["report", "--no-cache", "--no-recursive"])
        .arg(dir.path())
        .output()
        .unwrap();

    // Only one file at top level, so no duplicates
    assert!(output.status.success(), "expected exit code 0 (no dupes at top level)");
}

#[test]
fn empty_directory() {
    let dir = create_test_dir();

    let output = rsdedup_bin()
        .args(["report", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 0"));
    assert!(stdout.contains("Files scanned:    0"));
}
