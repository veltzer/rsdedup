use anyhow::{Result, bail};
use std::fs;
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

    for group in groups {
        let (keeper, to_remove) = split_group(&group.files, keep);
        for file in to_remove {
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
