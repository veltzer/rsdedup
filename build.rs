//! Build script: hands the git commit, branch and dirty state, the rustc
//! version and the build time to the crate as environment variables, for
//! the `version` command to report.

use std::process::Command;

/// Format a unix timestamp as "YYYY-MM-DD HH:MM:SS" (UTC) without external
/// dependencies (build.rs has none). Howard Hinnant's civil-from-days.
fn format_epoch(secs: u64) -> String {
    let days = (secs / 86_400).cast_signed();
    let rem = secs % 86_400;
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

/// `git <args>`'s trimmed stdout, or None when git fails (no repository,
/// as in `cargo package`'s verify build, or no git at all).
fn git_opt(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
}

fn git(args: &[&str]) -> String {
    git_opt(args).unwrap_or_else(|| "unknown".to_owned())
}

fn main() {
    let sha = git(&["rev-parse", "HEAD"]);
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"]);
    let describe = git(&["describe", "--tags", "--always"]);

    let rustc_ver = Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).to_string();
            s.split_whitespace().nth(1).map(str::to_owned)
        })
        .unwrap_or_else(|| "unknown".to_owned());

    // Only scan the [package] section: a bare `starts_with("edition")` scan
    // would match an `edition` key inside any later table first.
    let edition = std::fs::read_to_string("Cargo.toml")
        .ok()
        .and_then(|s| {
            let package_section = s.split("\n[").next().unwrap_or("");
            package_section
                .lines()
                .find(|l| l.trim_start().starts_with("edition"))
                .and_then(|l| l.split('=').nth(1))
                .map(|v| v.trim().trim_matches('"').to_owned())
        })
        .unwrap_or_else(|| "unknown".to_owned());
    // Tracked files that differ from HEAD. `git diff --quiet HEAD` answers
    // the same question but rewrites .git/index when it finds stat-dirty
    // entries, with or without --no-optional-locks; `status` honors the
    // flag, and a build script has no business writing there (a sandboxed
    // build rejects it as an undeclared write).
    let is_dirty = git_opt(&[
        "--no-optional-locks",
        "status",
        "--porcelain",
        "--untracked-files=no",
    ])
    .is_some_and(|out| !out.is_empty());
    let dirty_str = if is_dirty { "true" } else { "false" };
    let describe = if is_dirty {
        format!("{describe}-dirty")
    } else {
        describe
    };

    // In-process, and SOURCE_DATE_EPOCH-aware: forking `date(1)` embedded
    // the wall clock unconditionally, so two builds of the identical commit
    // produced different binaries and defeated content-addressed caching.
    // Reproducible-build environments set SOURCE_DATE_EPOCH; honor it.
    let epoch_secs = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs())
        });
    let build_timestamp = format_epoch(epoch_secs);
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");

    println!("cargo:rustc-env=RUST_EDITION={edition}");
    println!("cargo:rustc-env=GIT_SHA={sha}");
    println!("cargo:rustc-env=GIT_BRANCH={branch}");
    println!("cargo:rustc-env=GIT_DIRTY={dirty_str}");
    println!("cargo:rustc-env=RUSTC_SEMVER={rustc_ver}");
    println!("cargo:rustc-env=GIT_DESCRIBE={describe}");
    println!("cargo:rustc-env=BUILD_TIMESTAMP={build_timestamp}");

    println!("cargo:rerun-if-changed=Cargo.toml");
    // Where HEAD and the refs live, asked of git rather than assumed to be
    // `.git/`: in a linked worktree `.git` is a file, HEAD sits in the
    // worktree's own git dir and the refs in the common one.
    if let (Some(git_dir), Some(common_dir)) = (
        git_opt(&["rev-parse", "--git-dir"]),
        git_opt(&["rev-parse", "--git-common-dir"]),
    ) {
        println!("cargo:rerun-if-changed={git_dir}/HEAD");
        // Staging or committing flips GIT_DIRTY without touching a source.
        println!("cargo:rerun-if-changed={git_dir}/index");
        if let Ok(head) = std::fs::read_to_string(format!("{git_dir}/HEAD"))
            && let Some(refpath) = head.trim().strip_prefix("ref: ")
        {
            let loose = format!("{common_dir}/{refpath}");
            if std::path::Path::new(&loose).exists() {
                println!("cargo:rerun-if-changed={loose}");
            } else {
                println!("cargo:rerun-if-changed={common_dir}/packed-refs");
            }
        }
    }
    // GIT_DIRTY (and BUILD_TIMESTAMP with it) must follow edits to any
    // tracked file, not only ref moves; any rerun-if-changed line switches
    // off cargo's default of watching the whole package, so name them all.
    if let Some(files) = git_opt(&["ls-files"]) {
        for file in files.lines() {
            println!("cargo:rerun-if-changed={file}");
        }
    }
}
