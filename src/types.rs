use serde::{Deserialize, Serialize};
use std::fs::Metadata;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
    pub metadata: Metadata,
}

#[derive(Debug, Clone)]
pub struct DuplicateGroup {
    pub size: u64,
    pub hash: String,
    pub files: Vec<FileEntry>,
}

impl DuplicateGroup {
    pub fn wasted_bytes(&self) -> u64 {
        if self.files.len() <= 1 {
            return 0;
        }
        self.size * (self.files.len() as u64 - 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum HashAlgo {
    Sha256,
    Xxhash,
    Blake3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CompareMethod {
    SizeHash,
    Hash,
    ByteForByte,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum KeepStrategy {
    Interactive,
    First,
    Newest,
    Oldest,
    ShortestPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub size: u64,
    pub mtime_secs: i64,
    pub mtime_nanos: u32,
    pub inode: u64,
    pub hash_algo: String,
    pub partial_hash: Option<String>,
    pub full_hash: Option<String>,
    pub cached_at: u64,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub files_scanned: u64,
    pub duplicate_groups: u64,
    pub duplicate_files: u64,
    pub wasted_bytes: u64,
    pub action_taken: String,
    pub files_affected: u64,
    pub bytes_recovered: u64,
}
