use anyhow::{Context, Result};
use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{CacheEntry, HashAlgo};

/// Path -> JSON-encoded `CacheEntry`.
const HASHES: TableDefinition<&str, &[u8]> = TableDefinition::new("hashes");

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
    db: Database,
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
        // `cache.redb`, not the old sled `cache.db` directory: the formats are
        // unrelated and a cache is disposable, so the old one is simply left behind.
        let db_path = dir.join("cache.redb");
        let db = Database::create(&db_path)
            .with_context(|| format!("failed to open cache db: {}", db_path.display()))?;
        // Make sure the table exists so read transactions never fail on a fresh db.
        let txn = db.begin_write()?;
        txn.open_table(HASHES)?;
        txn.commit()?;
        Ok(Self { db, db_path })
    }

    fn get_raw(&self, key: &str) -> Option<CacheEntry> {
        let txn = self.db.begin_read().ok()?;
        let table = txn.open_table(HASHES).ok()?;
        let guard = table.get(key).ok()??;
        serde_json::from_slice(guard.value()).ok()
    }

    pub fn lookup(
        &self,
        path: &Path,
        algo: HashAlgo,
        metadata: &std::fs::Metadata,
    ) -> Option<CacheEntry> {
        let key = Self::make_key(path);
        let entry = self.get_raw(&key)?;

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

        let bytes = serde_json::to_vec(&entry)?;
        let key = Self::make_key(path);
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(HASHES)?;
            table.insert(key.as_str(), bytes.as_slice())?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        let txn = self.db.begin_write()?;
        txn.delete_table(HASHES)?;
        txn.open_table(HASHES)?;
        txn.commit()?;
        Ok(())
    }

    /// Every (path, entry) pair currently stored, decoded. Entries that fail
    /// to decode are skipped, as they were with the previous on-disk format.
    fn entries(&self) -> Result<Vec<(String, CacheEntry)>> {
        let txn = self.db.begin_read()?;
        let table = txn.open_table(HASHES)?;
        let mut out = Vec::new();
        for item in table.iter()? {
            let (key, value) = item?;
            if let Ok(entry) = serde_json::from_slice::<CacheEntry>(value.value()) {
                out.push((key.value().to_string(), entry));
            }
        }
        Ok(out)
    }

    pub fn stats(&self) -> Result<CacheStats> {
        let size = std::fs::metadata(&self.db_path)?.len();
        let entries = self.entries()?;
        let count = entries.len() as u64;

        let mut algo_counts: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        let mut oldest: Option<u64> = None;
        let mut newest: Option<u64> = None;
        let mut total_file_size: u64 = 0;
        let mut with_partial: u64 = 0;
        let mut with_full: u64 = 0;
        let mut stale: u64 = 0;

        for (path, entry) in &entries {
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
            if !std::path::Path::new(path).exists() {
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
        let stale: Vec<String> = {
            let txn = self.db.begin_read()?;
            let table = txn.open_table(HASHES)?;
            let mut keys = Vec::new();
            for item in table.iter()? {
                let (key, _) = item?;
                let path = key.value();
                if !std::path::Path::new(path).exists() {
                    keys.push(path.to_string());
                }
            }
            keys
        };
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(HASHES)?;
            for key in &stale {
                table.remove(key.as_str())?;
            }
        }
        txn.commit()?;
        Ok(stale.len() as u64)
    }

    pub fn iter(&self) -> impl Iterator<Item = (String, CacheEntry)> + '_ {
        self.entries().unwrap_or_default().into_iter()
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }

    /// Every write is committed durably by redb, so there is nothing to flush;
    /// kept so callers do not need to know which store is behind the cache.
    pub fn flush(&self) -> Result<()> {
        Ok(())
    }

    fn make_key(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }
}

fn algo_str(algo: HashAlgo) -> &'static str {
    match algo {
        HashAlgo::Sha256 => "sha256",
        HashAlgo::Xxhash => "xxhash",
        HashAlgo::Blake3 => "blake3",
    }
}
