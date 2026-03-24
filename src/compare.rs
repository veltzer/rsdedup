use anyhow::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::Mutex;

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

    // Wrap cache in a Mutex to serialize access from rayon threads,
    // avoiding sled's internal stack overflow under concurrent access.
    let cache_mutex = cache.map(Mutex::new);

    let results: Vec<Vec<DuplicateGroup>> = pool.install(|| {
        size_groups
            .into_par_iter()
            .map(|group| match method {
                CompareMethod::SizeHash => {
                    find_dupes_size_hash(group, algo, cache_mutex.as_ref())
                }
                CompareMethod::Hash => find_dupes_hash(group, algo, cache_mutex.as_ref()),
                CompareMethod::ByteForByte => find_dupes_byte_for_byte(group),
            })
            .filter_map(|r| r.ok())
            .collect()
    });

    Ok(results.into_iter().flatten().collect())
}

fn cache_lookup(
    cache_mutex: Option<&Mutex<&HashCache>>,
    entry: &FileEntry,
    algo: HashAlgo,
) -> (Option<String>, Option<String>) {
    let guard = match cache_mutex {
        Some(m) => match m.lock() {
            Ok(g) => g,
            Err(_) => return (None, None),
        },
        None => return (None, None),
    };
    match guard.lookup(&entry.path, algo, &entry.metadata) {
        Some(cached) => (cached.partial_hash, cached.full_hash),
        None => (None, None),
    }
}

fn cache_store(
    cache_mutex: Option<&Mutex<&HashCache>>,
    entry: &FileEntry,
    algo: HashAlgo,
    partial: Option<&str>,
    full: Option<&str>,
) {
    if let Some(m) = cache_mutex
        && let Ok(guard) = m.lock()
    {
        let _ = guard.store(&entry.path, algo, &entry.metadata, partial, full);
    }
}

fn find_dupes_size_hash(
    group: Vec<FileEntry>,
    algo: HashAlgo,
    cache_mutex: Option<&Mutex<&HashCache>>,
) -> Result<Vec<DuplicateGroup>> {
    // Phase 1: partial hash to narrow candidates
    let mut partial_map: HashMap<String, Vec<FileEntry>> = HashMap::new();
    for entry in group {
        let (cached_partial, _) = cache_lookup(cache_mutex, &entry, algo);
        let partial = cached_partial
            .or_else(|| {
                let h = hash_file(&entry.path, algo, true).ok()?;
                cache_store(cache_mutex, &entry, algo, Some(&h), None);
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
            let (_, cached_full) = cache_lookup(cache_mutex, &entry, algo);
            let full = cached_full
                .or_else(|| {
                    let h = hash_file(&entry.path, algo, false).ok()?;
                    cache_store(cache_mutex, &entry, algo, None, Some(&h));
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
    cache_mutex: Option<&Mutex<&HashCache>>,
) -> Result<Vec<DuplicateGroup>> {
    let mut hash_map: HashMap<String, Vec<FileEntry>> = HashMap::new();
    for entry in group {
        let (_, cached_full) = cache_lookup(cache_mutex, &entry, algo);
        let hash = cached_full
            .or_else(|| {
                let h = hash_file(&entry.path, algo, false).ok()?;
                cache_store(cache_mutex, &entry, algo, None, Some(&h));
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
