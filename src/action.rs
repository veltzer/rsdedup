use anyhow::{Result, bail};
use std::fs;
use std::io::{self, BufRead, Write};
use std::os::unix::fs::MetadataExt;

use crate::types::{DuplicateGroup, FileEntry, KeepStrategy};

pub struct ActionResult {
    pub files_affected: u64,
    pub bytes_recovered: u64,
}

pub fn delete_duplicates(
    groups: &[DuplicateGroup],
    keep: KeepStrategy,
    dry_run: bool,
    verbose: bool,
) -> Result<ActionResult> {
    let mut affected = 0u64;
    let mut recovered = 0u64;

    for (group_idx, group) in groups.iter().enumerate() {
        let (keeper_idx, to_remove_indices) = if keep == KeepStrategy::Interactive {
            prompt_interactive(group, group_idx + 1, groups.len())?
        } else {
            let (keeper, _) = split_group(&group.files, keep);
            let ki = group
                .files
                .iter()
                .position(|f| std::ptr::eq(f, keeper))
                .unwrap_or(0);
            let ri: Vec<usize> = (0..group.files.len()).filter(|i| *i != ki).collect();
            (ki, ri)
        };

        let keeper = &group.files[keeper_idx];
        for &idx in &to_remove_indices {
            let file = &group.files[idx];
            if dry_run {
                println!(
                    "would delete: {} (keeping {})",
                    file.path.display(),
                    keeper.path.display()
                );
            } else {
                if verbose {
                    println!(
                        "deleting: {} (keeping {})",
                        file.path.display(),
                        keeper.path.display()
                    );
                }
                fs::remove_file(&file.path)?;
            }
            affected += 1;
            recovered += file.size;
        }
    }

    Ok(ActionResult {
        files_affected: affected,
        bytes_recovered: recovered,
    })
}

fn prompt_interactive(
    group: &DuplicateGroup,
    group_num: usize,
    total_groups: usize,
) -> Result<(usize, Vec<usize>)> {
    // Build sorted indices by path
    let mut sorted_indices: Vec<usize> = (0..group.files.len()).collect();
    sorted_indices.sort_by(|a, b| group.files[*a].path.cmp(&group.files[*b].path));

    println!(
        "\n--- Duplicate group {}/{} (size: {} bytes, {} files) ---",
        group_num,
        total_groups,
        group.size,
        group.files.len()
    );
    for (display_num, &original_idx) in sorted_indices.iter().enumerate() {
        println!(
            "  [{}] {}",
            display_num + 1,
            group.files[original_idx].path.display()
        );
    }
    println!("  [s] Skip this group");

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    loop {
        print!("Keep which file? [1-{}|s]: ", group.files.len());
        stdout.flush()?;

        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        let input = line.trim();

        if input.eq_ignore_ascii_case("s") {
            // Skip: keep all files (remove none)
            return Ok((0, Vec::new()));
        }

        if let Ok(num) = input.parse::<usize>()
            && num >= 1
            && num <= group.files.len()
        {
            let keeper_original_idx = sorted_indices[num - 1];
            let to_remove: Vec<usize> = sorted_indices
                .iter()
                .copied()
                .filter(|&i| i != keeper_original_idx)
                .collect();
            return Ok((keeper_original_idx, to_remove));
        }

        println!("Invalid choice. Enter a number 1-{} or 's' to skip.", group.files.len());
    }
}

pub fn hardlink_duplicates(
    groups: &[DuplicateGroup],
    dry_run: bool,
    verbose: bool,
) -> Result<ActionResult> {
    let mut affected = 0u64;
    let mut recovered = 0u64;

    for group in groups {
        let (keeper, to_link) = split_group(&group.files, KeepStrategy::First);

        // Check all files are on the same filesystem
        let keeper_dev = keeper.metadata.dev();
        for file in &to_link {
            if file.metadata.dev() != keeper_dev {
                bail!(
                    "cannot hardlink across filesystems: {} and {}",
                    keeper.path.display(),
                    file.path.display()
                );
            }
        }

        for file in to_link {
            if dry_run {
                println!(
                    "would hardlink: {} -> {}",
                    file.path.display(),
                    keeper.path.display()
                );
            } else {
                if verbose {
                    println!(
                        "hardlinking: {} -> {}",
                        file.path.display(),
                        keeper.path.display()
                    );
                }
                fs::remove_file(&file.path)?;
                fs::hard_link(&keeper.path, &file.path)?;
            }
            affected += 1;
            recovered += file.size;
        }
    }

    Ok(ActionResult {
        files_affected: affected,
        bytes_recovered: recovered,
    })
}

pub fn symlink_duplicates(
    groups: &[DuplicateGroup],
    dry_run: bool,
    verbose: bool,
) -> Result<ActionResult> {
    let mut affected = 0u64;
    let mut recovered = 0u64;

    for group in groups {
        let (keeper, to_link) = split_group(&group.files, KeepStrategy::First);
        let keeper_abs = fs::canonicalize(&keeper.path)?;

        for file in to_link {
            if dry_run {
                println!(
                    "would symlink: {} -> {}",
                    file.path.display(),
                    keeper_abs.display()
                );
            } else {
                if verbose {
                    println!(
                        "symlinking: {} -> {}",
                        file.path.display(),
                        keeper_abs.display()
                    );
                }
                fs::remove_file(&file.path)?;
                std::os::unix::fs::symlink(&keeper_abs, &file.path)?;
            }
            affected += 1;
            recovered += file.size;
        }
    }

    Ok(ActionResult {
        files_affected: affected,
        bytes_recovered: recovered,
    })
}

fn split_group(files: &[FileEntry], strategy: KeepStrategy) -> (&FileEntry, Vec<&FileEntry>) {
    let keeper_idx = match strategy {
        KeepStrategy::Interactive => unreachable!("interactive mode handled separately"),
        KeepStrategy::First => 0,
        KeepStrategy::Newest => files
            .iter()
            .enumerate()
            .max_by_key(|(_, f)| f.metadata.modified().ok())
            .map(|(i, _)| i)
            .unwrap_or(0),
        KeepStrategy::Oldest => files
            .iter()
            .enumerate()
            .min_by_key(|(_, f)| f.metadata.modified().ok())
            .map(|(i, _)| i)
            .unwrap_or(0),
        KeepStrategy::ShortestPath => files
            .iter()
            .enumerate()
            .min_by_key(|(_, f)| f.path.as_os_str().len())
            .map(|(i, _)| i)
            .unwrap_or(0),
    };

    let keeper = &files[keeper_idx];
    let rest: Vec<&FileEntry> = files
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != keeper_idx)
        .map(|(_, f)| f)
        .collect();

    (keeper, rest)
}
