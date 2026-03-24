use std::process::Command;

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

    let edition = std::fs::read_to_string("Cargo.toml")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("edition"))
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

    let build_timestamp = Command::new("date")
        .arg("+%Y-%m-%d %H:%M:%S")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .unwrap_or_else(|| "unknown".to_owned());

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
