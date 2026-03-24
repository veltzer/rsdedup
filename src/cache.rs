use anyhow::{Context, Result};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{CacheEntry, HashAlgo};

pub struct CacheStats {
    pub entries: u64,
    pub db_size: u64,
    pub total_file_size: u64,
    pub with_partial: u64,
    pub with_full: u64,
    pub stale: u64,
    pub oldest_timestamp: Option<u64>,
    pub newest_timestamp: Option<u64>,
    pub algo_counts: std::collections::HashMap<String, u64>,
}

pub struct HashCache {
    db: sled::Db,
    db_path: PathBuf,
}

fn cache_dir() -> PathBuf {
    dirs_or_default()
}

fn dirs_or_default() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".rsdedup")
}

impl HashCache {
    pub fn open() -> Result<Self> {
        let dir = cache_dir();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create cache dir: {}", dir.display()))?;
        let db_path = dir.join("cache.db");
        let db = sled::open(&db_path)
            .with_context(|| format!("failed to open cache db: {}", db_path.display()))?;
        Ok(Self { db, db_path })
    }

    pub fn lookup(
        &self,
        path: &Path,
        algo: HashAlgo,
        metadata: &std::fs::Metadata,
    ) -> Option<CacheEntry> {
        let key = Self::make_key(path);
        let bytes = self.db.get(&key).ok()??;
        let entry: CacheEntry = bincode::deserialize(&bytes).ok()?;

        if entry.hash_algo != algo_str(algo) {
            return None;
        }

        let mtime = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;

        if entry.size != metadata.len()
            || entry.mtime_secs != mtime.as_secs() as i64
            || entry.mtime_nanos != mtime.subsec_nanos()
            || entry.inode != metadata.ino()
        {
            return None;
        }

        Some(entry)
    }

    pub fn store(
        &self,
        path: &Path,
        algo: HashAlgo,
        metadata: &std::fs::Metadata,
        partial_hash: Option<&str>,
        full_hash: Option<&str>,
    ) -> Result<()> {
        let mtime = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Merge with existing entry if the file hasn't changed
        let existing = self.lookup(path, algo, metadata);

        let entry = CacheEntry {
            size: metadata.len(),
            mtime_secs: mtime.as_secs() as i64,
            mtime_nanos: mtime.subsec_nanos(),
            inode: metadata.ino(),
            hash_algo: algo_str(algo).to_string(),
            partial_hash: partial_hash
                .map(|s| s.to_string())
                .or_else(|| existing.as_ref().and_then(|e| e.partial_hash.clone())),
            full_hash: full_hash
                .map(|s| s.to_string())
                .or_else(|| existing.as_ref().and_then(|e| e.full_hash.clone())),
            cached_at: now,
        };

        let bytes = bincode::serialize(&entry)?;
        let key = Self::make_key(path);
        self.db.insert(key, bytes)?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        self.db.clear()?;
        self.db.flush()?;
        Ok(())
    }

    pub fn stats(&self) -> Result<CacheStats> {
        let count = self.db.len() as u64;
        let size = self.db.size_on_disk()?;

        let mut algo_counts: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        let mut oldest: Option<u64> = None;
        let mut newest: Option<u64> = None;
        let mut total_file_size: u64 = 0;
        let mut with_partial: u64 = 0;
        let mut with_full: u64 = 0;
        let mut stale: u64 = 0;

        for item in self.db.iter() {
            let (key, value) = match item {
                Ok(kv) => kv,
                Err(_) => continue,
            };

            let entry: CacheEntry = match bincode::deserialize(&value) {
                Ok(e) => e,
                Err(_) => continue,
            };

            *algo_counts.entry(entry.hash_algo.clone()).or_default() += 1;
            total_file_size += entry.size;

            if entry.partial_hash.is_some() {
                with_partial += 1;
            }
            if entry.full_hash.is_some() {
                with_full += 1;
            }

            let ts = entry.cached_at;
            oldest = Some(oldest.map_or(ts, |o: u64| o.min(ts)));
            newest = Some(newest.map_or(ts, |n: u64| n.max(ts)));

            // Check if the file still exists
            let path = String::from_utf8_lossy(&key);
            if !std::path::Path::new(path.as_ref()).exists() {
                stale += 1;
            }
        }

        Ok(CacheStats {
            entries: count,
            db_size: size,
            total_file_size,
            with_partial,
            with_full,
            stale,
            oldest_timestamp: oldest,
            newest_timestamp: newest,
            algo_counts,
        })
    }

    pub fn prune(&self) -> Result<u64> {
        let mut removed = 0u64;
        for item in self.db.iter() {
            let (key, _) = match item {
                Ok(kv) => kv,
                Err(_) => continue,
            };
            let path = String::from_utf8_lossy(&key);
            if !std::path::Path::new(path.as_ref()).exists() {
                self.db.remove(&key)?;
                removed += 1;
            }
        }
        self.db.flush()?;
        Ok(removed)
    }

    pub fn iter(&self) -> impl Iterator<Item = (String, CacheEntry)> + '_ {
        self.db.iter().filter_map(|item| {
            let (key, value) = item.ok()?;
            let path = String::from_utf8_lossy(&key).into_owned();
            let entry: CacheEntry = bincode::deserialize(&value).ok()?;
            Some((path, entry))
        })
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }

    pub fn flush(&self) -> Result<()> {
        self.db.flush()?;
        Ok(())
    }

    fn make_key(path: &Path) -> Vec<u8> {
        path.to_string_lossy().as_bytes().to_vec()
    }
}

fn algo_str(algo: HashAlgo) -> &'static str {
    match algo {
        HashAlgo::Sha256 => "sha256",
        HashAlgo::Xxhash => "xxhash",
        HashAlgo::Blake3 => "blake3",
    }
}
