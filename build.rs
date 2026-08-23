use std::process::Command;

/// Format a unix timestamp as "YYYY-MM-DD HH:MM:SS" (UTC) without external
/// dependencies (build.rs has none). Howard Hinnant's civil-from-days.
fn format_epoch(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, min, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02} {h:02}:{min:02}:{s:02}")
}

fn git(args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned())
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
            s.split_whitespace().nth(1).map(|v| v.to_owned())
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
    let is_dirty = Command::new("git")
        .args(["diff", "--quiet", "HEAD"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| !s.success());
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
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Ok(head) = std::fs::read_to_string(".git/HEAD")
        && let Some(refpath) = head.trim().strip_prefix("ref: ")
    {
        let loose = format!(".git/{refpath}");
        if std::path::Path::new(&loose).exists() {
            println!("cargo:rerun-if-changed={loose}");
        } else {
            println!("cargo:rerun-if-changed=.git/packed-refs");
        }
    }
}
