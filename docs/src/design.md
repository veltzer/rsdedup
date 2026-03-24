# Design

## Overview

`rsdedup` is a fast, Rust-based file deduplication tool. It scans directories for duplicate files and supports multiple actions: reporting, hardlinking/symlinking, and deleting duplicates (keeping one copy).

## Goals

- Fast duplicate detection across large directory trees
- Multiple comparison strategies (hash-based, size+hash, byte-for-byte)
- Multiple actions on duplicates (report, hardlink, symlink, delete)
- Safe defaults — report-only unless explicitly told to modify files
- Parallel file hashing for performance

## Pipeline Architecture

rsdedup processes files through a multi-stage pipeline where each stage reduces the candidate set:

```
1. Scan       →  Walk directories, collect file metadata
2. Group      →  Group files by size (unique sizes eliminated)
3. Filter     →  Apply min-size, max-size, include/exclude filters
4. Compare    →  Compare candidates using the chosen strategy
5. Act        →  Perform the chosen action on duplicate groups
```

## Module Structure

```
src/
├── main.rs       — Orchestration and command dispatch
├── cli.rs        — CLI definitions (clap derive)
├── scanner.rs    — Directory walking with walkdir
├── grouper.rs    — Group files by size
├── compare.rs    — Comparison strategies (size-hash, hash, byte-for-byte)
├── hasher.rs     — Hash implementations (SHA-256, xxHash, BLAKE3)
├── cache.rs      — Persistent hash cache (sled)
├── action.rs     — Actions: report, delete, hardlink, symlink
├── output.rs     — Output formatting (text, JSON)
├── types.rs      — Shared types
└── error.rs      — Exit codes
```

## Key Types

```rust
struct FileEntry {
    path: PathBuf,
    size: u64,
    metadata: Metadata,
}

struct DuplicateGroup {
    size: u64,
    hash: String,
    files: Vec<FileEntry>,
}

enum CompareMethod {
    SizeHash,
    Hash,
    ByteForByte,
}

enum KeepStrategy {
    First,
    Newest,
    Oldest,
    ShortestPath,
}
```

## Parallelism

- Directory walking is single-threaded (I/O bound, using `walkdir`)
- File comparison uses a `rayon` thread pool — size groups are processed in parallel
- Within a single size group, files are hashed and compared sequentially
- Thread count is configurable via `--jobs` (defaults to CPU core count)

See the [Parallelism](parallelism.md) chapter for details on controlling thread count and when parallelism helps most.

## Cache Design

The hash cache uses `sled`, an embedded key-value store at `~/.rsdedup/cache.db`. Each entry maps a file path to its metadata (size, mtime, inode) and hash values (partial and full). Cache entries are invalidated when any metadata field changes. The cache merges partial and full hashes — computing one doesn't overwrite the other.

See the [Hash Cache](cache.md) chapter for details.

## Safety

- **Default action is report-only** — no files are modified unless explicitly requested
- **`--dry-run`** shows what would happen without making changes
- **No cross-filesystem hardlinks** — detected and reported as errors
- **Symlink loops** are avoided by not following symlinks by default

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Success, no duplicates found |
| `1` | Success, duplicates found |
| `2` | Error |

This makes rsdedup scriptable (e.g. `rsdedup report && echo "clean"`).

## Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing |
| `clap_complete` | Shell completion generation |
| `walkdir` | Recursive directory traversal |
| `rayon` | Parallel hashing |
| `sha2` | SHA-256 |
| `xxhash-rust` | xxHash (xxh3-128) |
| `blake3` | BLAKE3 |
| `sled` | Embedded key-value cache |
| `bincode` | Cache entry serialization |
| `serde` / `serde_json` | JSON output |
| `globset` | Include/exclude glob matching |
| `anyhow` | Error handling |
