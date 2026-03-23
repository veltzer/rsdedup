use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::path::Path;
use walkdir::WalkDir;

use crate::types::FileEntry;

pub struct ScanOptions {
    pub recursive: bool,
    pub follow_symlinks: bool,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

fn build_globset(patterns: &[String]) -> Result<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern).with_context(|| format!("invalid glob: {pattern}"))?);
    }
    Ok(Some(builder.build()?))
}

pub fn scan(path: &Path, opts: &ScanOptions) -> Result<Vec<FileEntry>> {
    let include_set = build_globset(&opts.include)?;
    let exclude_set = build_globset(&opts.exclude)?;

    let max_depth = if opts.recursive { usize::MAX } else { 1 };

    let walker = WalkDir::new(path)
        .max_depth(max_depth)
        .follow_links(opts.follow_symlinks);

    let mut entries = Vec::new();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("warning: {err}");
                continue;
            }
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let file_path = entry.path();
        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if let Some(ref exc) = exclude_set
            && (exc.is_match(&file_name) || exc.is_match(file_path))
        {
            continue;
        }

        if let Some(ref inc) = include_set
            && !inc.is_match(&file_name)
            && !inc.is_match(file_path)
        {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(err) => {
                eprintln!(
                    "warning: cannot read metadata for {}: {err}",
                    file_path.display()
                );
                continue;
            }
        };

        let size = metadata.len();

        if let Some(min) = opts.min_size
            && size < min
        {
            continue;
        }
        if let Some(max) = opts.max_size
            && size > max
        {
            continue;
        }

        entries.push(FileEntry {
            path: file_path.to_path_buf(),
            size,
            metadata,
        });
    }

    Ok(entries)
}
