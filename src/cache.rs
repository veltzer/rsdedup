use anyhow::{Context, Result};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{CacheEntry, HashAlgo};

pub struct HashCache {
    db: sled::Db,
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
        Ok(Self { db })
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

        let entry = CacheEntry {
            size: metadata.len(),
            mtime_secs: mtime.as_secs() as i64,
            mtime_nanos: mtime.subsec_nanos(),
            inode: metadata.ino(),
            hash_algo: algo_str(algo).to_string(),
            partial_hash: partial_hash.map(|s| s.to_string()),
            full_hash: full_hash.map(|s| s.to_string()),
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

    pub fn stats(&self) -> Result<(u64, u64)> {
        let count = self.db.len() as u64;
        let size = self.db.size_on_disk()?;
        Ok((count, size))
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
