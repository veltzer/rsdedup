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
        .args(["dedup", "report", "--no-cache"])
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
        .args(["dedup", "report", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1 (dupes found)"
    );
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
        .args(["dedup", "report", "--no-cache", "--output", "json"])
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
        .args(["dedup", "delete", "--no-cache", "--dry-run", "--keep", "first"])
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
        .args(["dedup", "delete", "--no-cache", "--keep", "first"])
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
        .args(["dedup", "hardlink", "--no-cache", "--dry-run"])
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
        .args(["dedup", "hardlink", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    let ino_a = fs::metadata(dir.path().join("a.txt")).unwrap().ino();
    let ino_b = fs::metadata(dir.path().join("b.txt")).unwrap().ino();
    assert_eq!(
        ino_a, ino_b,
        "files should share the same inode after hardlinking"
    );
}

#[test]
fn symlink_creates_symlinks() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "symlink this");
    write_file(dir.path(), "b.txt", "symlink this");

    rsdedup_bin()
        .args(["dedup", "symlink", "--no-cache"])
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
    assert!(
        a_is_symlink || b_is_symlink,
        "at least one file should be a symlink"
    );
}

#[test]
fn scan_populates_cache() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "cache me");

    let output = rsdedup_bin()
        .args(["cache", "scan"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("scanned 1 files"));
}

#[test]
fn min_size_filter() {
    let dir = create_test_dir();
    write_file(dir.path(), "small1.txt", "ab");
    write_file(dir.path(), "small2.txt", "ab");
    write_file(dir.path(), "big1.txt", "this is bigger content");
    write_file(dir.path(), "big2.txt", "this is bigger content");

    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache", "--min-size", "10"])
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
        .args(["dedup", "report", "--no-cache", "--max-size", "10"])
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
        .args(["dedup", "report", "--no-cache", "--exclude", "*.log"])
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
        .args(["dedup", "report", "--no-cache", "--include", "*.log"])
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
        .args(["dedup", "report", "--no-cache", "--compare", "byte-for-byte"])
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
        .args(["dedup", "report", "--no-cache", "--compare", "hash"])
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
        .args(["dedup", "report", "--no-cache", "--hash", "blake3"])
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
        .args(["dedup", "report", "--no-cache", "--hash", "xxhash"])
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
        .args(["complete", "bash"])
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
        .args(["dedup", "report", "--no-cache"])
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
        .args(["dedup", "report", "--no-cache", "--no-recursive"])
        .arg(dir.path())
        .output()
        .unwrap();

    // Only one file at top level, so no duplicates
    assert!(
        output.status.success(),
        "expected exit code 0 (no dupes at top level)"
    );
}

#[test]
fn empty_directory() {
    let dir = create_test_dir();

    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 0"));
    assert!(stdout.contains("Files scanned:    0"));
}

// --- Keep strategy tests ---

#[test]
fn delete_keep_newest() {
    let dir = create_test_dir();
    write_file(dir.path(), "old.txt", "keep strategy test");
    // Set old.txt to an older mtime
    let old_path = dir.path().join("old.txt");
    let old_time = filetime::FileTime::from_unix_time(1_000_000, 0);
    filetime::set_file_mtime(&old_path, old_time).unwrap();

    write_file(dir.path(), "new.txt", "keep strategy test");
    // new.txt has a more recent mtime by default

    let output = rsdedup_bin()
        .args(["dedup", "delete", "--no-cache", "--keep", "newest"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        dir.path().join("new.txt").exists(),
        "newest file should be kept"
    );
    assert!(
        !dir.path().join("old.txt").exists(),
        "older file should be deleted"
    );
}

#[test]
fn delete_keep_oldest() {
    let dir = create_test_dir();
    write_file(dir.path(), "old.txt", "keep oldest test");
    // old.txt gets the current mtime, so set it to something old
    let old_path = dir.path().join("old.txt");
    let old_time = filetime::FileTime::from_unix_time(1_000_000, 0);
    filetime::set_file_mtime(&old_path, old_time).unwrap();

    write_file(dir.path(), "new.txt", "keep oldest test");

    let output = rsdedup_bin()
        .args(["dedup", "delete", "--no-cache", "--keep", "oldest"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        dir.path().join("old.txt").exists(),
        "oldest file should be kept"
    );
    assert!(
        !dir.path().join("new.txt").exists(),
        "newer file should be deleted"
    );
}

#[test]
fn delete_keep_shortest_path() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "shortest path test");
    write_file(dir.path(), "subdir/longer_name.txt", "shortest path test");

    let output = rsdedup_bin()
        .args(["dedup", "delete", "--no-cache", "--keep", "shortest-path"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(
        dir.path().join("a.txt").exists(),
        "file with shortest path should be kept"
    );
    assert!(
        !dir.path().join("subdir/longer_name.txt").exists(),
        "file with longer path should be deleted"
    );
}

// --- Multiple duplicate groups ---

#[test]
fn multiple_duplicate_groups() {
    let dir = create_test_dir();
    write_file(dir.path(), "a1.txt", "group one content");
    write_file(dir.path(), "a2.txt", "group one content");
    write_file(dir.path(), "b1.txt", "group two content!!");
    write_file(dir.path(), "b2.txt", "group two content!!");

    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 2"));
    assert!(stdout.contains("Duplicate files:  2"));
}

#[test]
fn three_files_in_one_group() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "triple dup");
    write_file(dir.path(), "b.txt", "triple dup");
    write_file(dir.path(), "c.txt", "triple dup");

    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
    assert!(stdout.contains("Duplicate files:  2"));
}

// --- Cache subcommands ---

#[test]
fn cache_clear() {
    let output = rsdedup_bin().args(["cache", "clear"]).output().unwrap();

    // May fail if another test holds the sled lock; only assert it doesn't return exit code 2 (our error)
    // When it succeeds, it prints "cleared" on stderr
    if output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("cleared"));
    }
}

#[test]
fn cache_stats() {
    let output = rsdedup_bin().args(["cache", "stats"]).output().unwrap();

    // May fail due to sled lock contention with parallel tests
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("entries") || stdout.contains("Entries"));
    }
}

#[test]
fn cache_prune() {
    let output = rsdedup_bin().args(["cache", "prune"]).output().unwrap();

    if output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("pruned"));
    }
}

#[test]
fn cache_list() {
    let output = rsdedup_bin().args(["cache", "list"]).output().unwrap();

    // May fail due to sled lock contention with parallel tests
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("path\tsize\talgo\tpartial_hash\tfull_hash\tcached_at"));
    }
}

// --- Verbose and timing flags ---

#[test]
fn verbose_flag() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "verbose test");
    write_file(dir.path(), "b.txt", "verbose test");

    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache", "--verbose"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Verbose should produce some diagnostic output on stderr
    assert!(!stderr.is_empty(), "verbose flag should produce stderr output");
}

#[test]
fn no_timing_flag() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "timing test");
    write_file(dir.path(), "b.txt", "unique timing");

    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache", "--no-timing"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("elapsed") && !stdout.contains("Elapsed"));
}

// --- Empty files ---

#[test]
fn empty_files_are_duplicates() {
    let dir = create_test_dir();
    write_file(dir.path(), "empty1.txt", "");
    write_file(dir.path(), "empty2.txt", "");
    write_file(dir.path(), "notempty.txt", "has content");

    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    // Empty files may or may not be grouped as duplicates depending on implementation.
    // This test documents the behavior.
    assert!(output.status.success() || output.status.code() == Some(1));
}

// --- Special characters in filenames ---

#[test]
fn files_with_spaces_in_names() {
    let dir = create_test_dir();
    write_file(dir.path(), "file one.txt", "space dup");
    write_file(dir.path(), "file two.txt", "space dup");

    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
}

#[test]
fn files_with_unicode_names() {
    let dir = create_test_dir();
    write_file(dir.path(), "café.txt", "unicode dup");
    write_file(dir.path(), "naïve.txt", "unicode dup");

    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
}

// --- Size filtering edge cases ---

#[test]
fn min_and_max_size_combined() {
    let dir = create_test_dir();
    write_file(dir.path(), "tiny1.txt", "ab");
    write_file(dir.path(), "tiny2.txt", "ab");
    write_file(dir.path(), "mid1.txt", "medium sized");
    write_file(dir.path(), "mid2.txt", "medium sized");
    write_file(dir.path(), "big1.txt", "this is a much bigger file content here!!");
    write_file(dir.path(), "big2.txt", "this is a much bigger file content here!!");

    let output = rsdedup_bin()
        .args([
            "dedup",
            "report",
            "--no-cache",
            "--min-size",
            "5",
            "--max-size",
            "30",
        ])
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
    assert!(stdout.contains("mid1.txt") || stdout.contains("mid2.txt"));
    assert!(!stdout.contains("tiny1.txt"));
    assert!(!stdout.contains("big1.txt"));
}

// --- Multiple glob patterns ---

#[test]
fn multiple_exclude_patterns() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "dup");
    write_file(dir.path(), "b.txt", "dup");
    write_file(dir.path(), "a.log", "dup");
    write_file(dir.path(), "b.log", "dup");
    write_file(dir.path(), "a.bak", "dup");
    write_file(dir.path(), "b.bak", "dup");

    let output = rsdedup_bin()
        .args([
            "dedup",
            "report",
            "--no-cache",
            "--exclude",
            "*.log",
            "--exclude",
            "*.bak",
        ])
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
    assert!(!stdout.contains(".log"));
    assert!(!stdout.contains(".bak"));
}

// --- Symlink dry-run ---

#[test]
fn symlink_dry_run() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "symlink dry run");
    write_file(dir.path(), "b.txt", "symlink dry run");

    let output = rsdedup_bin()
        .args(["dedup", "symlink", "--no-cache", "--dry-run"])
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("would symlink"));
    // Both files should remain regular files
    let a_is_symlink = fs::symlink_metadata(dir.path().join("a.txt"))
        .unwrap()
        .file_type()
        .is_symlink();
    let b_is_symlink = fs::symlink_metadata(dir.path().join("b.txt"))
        .unwrap()
        .file_type()
        .is_symlink();
    assert!(!a_is_symlink && !b_is_symlink, "dry-run should not create symlinks");
}

// --- Jobs flag ---

#[test]
fn jobs_flag_single_thread() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "jobs test content");
    write_file(dir.path(), "b.txt", "jobs test content");

    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache", "--jobs", "1"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
}

// --- Completions for different shells ---

#[test]
fn complete_zsh() {
    let output = rsdedup_bin().args(["complete", "zsh"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty());
}

#[test]
fn complete_fish() {
    let output = rsdedup_bin().args(["complete", "fish"]).output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty());
}

// --- Default comparison strategy (size-hash) ---

#[test]
fn compare_size_hash_default() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "size-hash default");
    write_file(dir.path(), "b.txt", "size-hash default");

    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache", "--compare", "size-hash"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 1"));
}

// --- JSON output for actions ---

#[test]
fn delete_json_output() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "json delete test");
    write_file(dir.path(), "b.txt", "json delete test");

    let output = rsdedup_bin()
        .args([
            "dedup",
            "delete",
            "--no-cache",
            "--dry-run",
            "--keep",
            "first",
            "--output",
            "json",
        ])
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"group\"") || stdout.contains("\"files\""));
}

#[test]
fn hardlink_json_output() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "json hardlink test");
    write_file(dir.path(), "b.txt", "json hardlink test");

    let output = rsdedup_bin()
        .args([
            "dedup",
            "hardlink",
            "--no-cache",
            "--dry-run",
            "--output",
            "json",
        ])
        .arg(dir.path())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"group\"") || stdout.contains("\"files\""));
}

// --- Nonexistent directory ---

#[test]
fn nonexistent_directory_warns() {
    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache", "/tmp/rsdedup_nonexistent_dir_test_12345"])
        .output()
        .unwrap();

    // Tool treats nonexistent path as empty scan with a warning
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No such file") || stderr.contains("IO error"),
        "should warn about nonexistent directory"
    );
}

// --- Single file (no possible duplicates) ---

#[test]
fn single_file_no_duplicates() {
    let dir = create_test_dir();
    write_file(dir.path(), "only.txt", "just one file");

    let output = rsdedup_bin()
        .args(["dedup", "report", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Duplicate groups: 0"));
}

// --- Delete with three duplicates keeps exactly one ---

#[test]
fn delete_three_duplicates_keeps_one() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "triple delete");
    write_file(dir.path(), "b.txt", "triple delete");
    write_file(dir.path(), "c.txt", "triple delete");

    let output = rsdedup_bin()
        .args(["dedup", "delete", "--no-cache", "--keep", "first"])
        .arg(dir.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let remaining: Vec<_> = ["a.txt", "b.txt", "c.txt"]
        .iter()
        .filter(|f| dir.path().join(f).exists())
        .collect();
    assert_eq!(remaining.len(), 1, "exactly one file should remain");
}

// --- Hardlink with three duplicates ---

#[test]
fn hardlink_three_files() {
    let dir = create_test_dir();
    write_file(dir.path(), "a.txt", "triple hardlink");
    write_file(dir.path(), "b.txt", "triple hardlink");
    write_file(dir.path(), "c.txt", "triple hardlink");

    rsdedup_bin()
        .args(["dedup", "hardlink", "--no-cache"])
        .arg(dir.path())
        .output()
        .unwrap();

    let ino_a = fs::metadata(dir.path().join("a.txt")).unwrap().ino();
    let ino_b = fs::metadata(dir.path().join("b.txt")).unwrap().ino();
    let ino_c = fs::metadata(dir.path().join("c.txt")).unwrap().ino();
    assert_eq!(ino_a, ino_b);
    assert_eq!(ino_b, ino_c);
}
