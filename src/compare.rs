use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;

use crate::cache::HashCache;
use crate::hasher::hash_file;
use crate::types::{CompareMethod, DuplicateGroup, FileEntry, HashAlgo};

pub fn find_duplicates(
    size_groups: Vec<Vec<FileEntry>>,
    method: CompareMethod,
    algo: HashAlgo,
    cache: Option<&HashCache>,
    num_jobs: usize,
) -> Result<Vec<DuplicateGroup>> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_jobs)
        .build()?;

    let results: Vec<Vec<DuplicateGroup>> = pool.install(|| {
        size_groups
            .into_par_iter()
            .map(|group| match method {
                CompareMethod::SizeHash => find_dupes_size_hash(group, algo, cache),
                CompareMethod::Hash => find_dupes_hash(group, algo, cache),
                CompareMethod::ByteForByte => find_dupes_byte_for_byte(group),
            })
            .filter_map(|r| r.ok())
            .collect()
    });

    Ok(results.into_iter().flatten().collect())
}

fn find_dupes_size_hash(
    group: Vec<FileEntry>,
    algo: HashAlgo,
    cache: Option<&HashCache>,
) -> Result<Vec<DuplicateGroup>> {
    // Phase 1: partial hash to narrow candidates
    let mut partial_map: HashMap<String, Vec<FileEntry>> = HashMap::new();
    for entry in group {
        let partial = get_cached_partial(&entry, algo, cache)
            .or_else(|| {
                let h = hash_file(&entry.path, algo, true).ok()?;
                if let Some(c) = cache {
                    let _ = c.store(&entry.path, algo, &entry.metadata, Some(&h), None);
                }
                Some(h)
            })
            .unwrap_or_default();
        partial_map.entry(partial).or_default().push(entry);
    }

    // Phase 2: full hash for groups that survived partial
    let mut result = Vec::new();
    for (_partial, candidates) in partial_map {
        if candidates.len() < 2 {
            continue;
        }
        let mut full_map: HashMap<String, Vec<FileEntry>> = HashMap::new();
        for entry in candidates {
            let full = get_cached_full(&entry, algo, cache)
                .or_else(|| {
                    let h = hash_file(&entry.path, algo, false).ok()?;
                    if let Some(c) = cache {
                        let _ = c.store(&entry.path, algo, &entry.metadata, None, Some(&h));
                    }
                    Some(h)
                })
                .unwrap_or_default();
            full_map.entry(full).or_default().push(entry);
        }
        for (hash, files) in full_map {
            if files.len() > 1 {
                result.push(DuplicateGroup {
                    size: files[0].size,
                    hash,
                    files,
                });
            }
        }
    }

    Ok(result)
}

fn find_dupes_hash(
    group: Vec<FileEntry>,
    algo: HashAlgo,
    cache: Option<&HashCache>,
) -> Result<Vec<DuplicateGroup>> {
    let mut hash_map: HashMap<String, Vec<FileEntry>> = HashMap::new();
    for entry in group {
        let hash = get_cached_full(&entry, algo, cache)
            .or_else(|| {
                let h = hash_file(&entry.path, algo, false).ok()?;
                if let Some(c) = cache {
                    let _ = c.store(&entry.path, algo, &entry.metadata, None, Some(&h));
                }
                Some(h)
            })
            .unwrap_or_default();
        hash_map.entry(hash).or_default().push(entry);
    }

    Ok(hash_map
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(hash, files)| DuplicateGroup {
            size: files[0].size,
            hash,
            files,
        })
        .collect())
}

fn find_dupes_byte_for_byte(group: Vec<FileEntry>) -> Result<Vec<DuplicateGroup>> {
    let mut groups: Vec<Vec<FileEntry>> = Vec::new();

    'outer: for entry in group {
        for existing_group in &mut groups {
            if files_equal(&entry.path, &existing_group[0].path)? {
                existing_group.push(entry);
                continue 'outer;
            }
        }
        groups.push(vec![entry]);
    }

    Ok(groups
        .into_iter()
        .filter(|g| g.len() > 1)
        .map(|files| DuplicateGroup {
            size: files[0].size,
            hash: "byte-for-byte".to_string(),
            files,
        })
        .collect())
}

fn files_equal(a: &std::path::Path, b: &std::path::Path) -> Result<bool> {
    let mut fa = File::open(a)?;
    let mut fb = File::open(b)?;
    let mut buf_a = [0u8; 65536];
    let mut buf_b = [0u8; 65536];
    loop {
        let na = fa.read(&mut buf_a)?;
        let nb = fb.read(&mut buf_b)?;
        if na != nb || buf_a[..na] != buf_b[..nb] {
            return Ok(false);
        }
        if na == 0 {
            return Ok(true);
        }
    }
}

fn get_cached_partial(
    entry: &FileEntry,
    algo: HashAlgo,
    cache: Option<&HashCache>,
) -> Option<String> {
    let c = cache?;
    let cached = c.lookup(&entry.path, algo, &entry.metadata)?;
    cached.partial_hash
}

fn get_cached_full(entry: &FileEntry, algo: HashAlgo, cache: Option<&HashCache>) -> Option<String> {
    let c = cache?;
    let cached = c.lookup(&entry.path, algo, &entry.metadata)?;
    cached.full_hash
}
