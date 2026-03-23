# rsdedup — Design Document

## Overview

`rsdedup` is a fast, Rust-based file deduplication tool. It scans directories for duplicate files and supports multiple actions: reporting, hardlinking/symlinking, and deleting duplicates (keeping one copy).

## Goals

- Fast duplicate detection across large directory trees
- Multiple comparison strategies (hash-based, size+hash, byte-for-byte)
- Multiple actions on duplicates (report, hardlink, symlink, delete)
- Safe defaults — report-only unless explicitly told to modify files
- Parallel file scanning and hashing for performance

## CLI Interface

```
rsdedup <COMMAND> [OPTIONS] [PATH]
```

By default, all scan subcommands operate on the current working directory. Use `--path <PATH>` or a positional argument to override.

### Global Options

These apply to all subcommands that scan files:

| Flag | Description |
|------|-------------|
| `--compare <METHOD>` | Comparison method: `size-hash` (default), `hash`, `byte-for-byte` |
| `--hash <ALGO>` | Hash algorithm: `sha256` (default), `xxhash`, `blake3` |
| `--min-size <SIZE>` | Minimum file size to consider (default: 1 byte) |
| `--max-size <SIZE>` | Maximum file size to consider (default: unlimited) |
| `--recursive` / `-r` | Recurse into subdirectories (default: true) |
| `--no-recursive` | Do not recurse into subdirectories |
| `--follow-symlinks` | Follow symbolic links (default: false) |
| `--verbose` / `-v` | Verbose output |
| `--output <FORMAT>` | Output format: `text` (default), `json`, `csv` |
| `--jobs <N>` / `-j` | Number of parallel workers (default: number of CPUs) |
| `--path <PATH>` / `-p` | Directory to scan (default: current working directory) |
| `--no-cache` | Disable the hash cache, recompute all hashes |

### Subcommands

#### `rsdedup report [PATH]`

Find and report duplicate files. No files are modified. This is the read-only, safe default.

#### `rsdedup delete [PATH]`

Delete duplicate files, keeping one copy per group.

| Flag | Description |
|------|-------------|
| `--keep <STRATEGY>` | Which file to keep: `first` (default), `newest`, `oldest`, `shortest-path` |
| `--dry-run` / `-n` | Show what would be deleted without deleting |

#### `rsdedup hardlink [PATH]`

Replace duplicate files with hardlinks to a single copy.

| Flag | Description |
|------|-------------|
| `--dry-run` / `-n` | Show what would be hardlinked without making changes |

#### `rsdedup symlink [PATH]`

Replace duplicate files with symlinks to a single copy.

| Flag | Description |
|------|-------------|
| `--dry-run` / `-n` | Show what would be symlinked without making changes |

#### `rsdedup scan [PATH]`

Scan a directory and populate the hash cache without performing any dedup action. Useful for warming up the cache ahead of time so that subsequent `report`/`delete`/`hardlink`/`symlink` runs are faster.

#### `rsdedup cache <ACTION>`

Manage the hash cache.

| Action | Description |
|--------|-------------|
| `clear` | Delete the cache database |
| `stats` | Show cache size, entry count, and age statistics |

### Examples

```bash
# Report duplicates in current directory
rsdedup report

# Report duplicates in a specific directory
rsdedup report /home/user/photos

# Delete duplicates, keeping the oldest file
rsdedup delete --keep oldest /home/user/photos

# Hardlink duplicates in current directory, dry-run first
rsdedup hardlink --dry-run

# Use byte-for-byte comparison with JSON output
rsdedup report --compare byte-for-byte --output json /backup

# Pre-populate the cache for a directory
rsdedup scan /home/user/photos

# Show cache statistics
rsdedup cache stats

# Clear the cache
rsdedup cache clear
```

## Architecture

### Pipeline

The deduplication process is a multi-stage pipeline, where each stage reduces the candidate set:

```
1. Scan       →  Walk directories, collect file metadata
2. Group      →  Group files by size (files with unique sizes can't be duplicates)
3. Filter     →  Apply min-size/max-size filters
4. Compare    →  Compare files within each size group using the chosen method
5. Act        →  Perform the chosen action on each duplicate group
```

### Modules

```
src/
├── main.rs          # CLI parsing (clap), orchestration
├── scanner.rs       # Directory walking, metadata collection
├── grouper.rs       # Group files by size
├── compare.rs       # Comparison strategies (hash, byte-for-byte)
├── hasher.rs        # Hashing implementations (SHA256, xxHash, BLAKE3)
├── cache.rs         # Hash cache (SQLite in ~/.rsdedup/cache.db)
├── action.rs        # Actions: report, hardlink, symlink, delete
├── types.rs         # Shared types (FileEntry, DuplicateGroup, etc.)
├── output.rs        # Output formatting (text, JSON, CSV)
└── error.rs         # Error types
```

### Key Types

```rust
struct FileEntry {
    path: PathBuf,
    size: u64,
    metadata: Metadata,
}

struct DuplicateGroup {
    size: u64,
    files: Vec<FileEntry>,
}

enum Action {
    Report,
    Hardlink,
    Symlink,
    Delete { keep: KeepStrategy },
}

enum KeepStrategy {
    First,
    Newest,
    Oldest,
    ShortestPath,
}

enum CompareMethod {
    Hash,
    SizeHash,
    ByteForByte,
}
```

### Comparison Strategy: `size-hash` (default)

This is a two-phase approach for best performance:

1. **Size grouping** — files with unique sizes are immediately excluded
2. **Partial hash** — hash only the first 4KB of each file; files with unique partial hashes are excluded
3. **Full hash** — hash the entire file for remaining candidates

This avoids reading entire files when a quick check can rule out matches.

### Hash Cache

To avoid rehashing files that haven't changed, rsdedup maintains a persistent cache at `~/.rsdedup/cache.db` (SQLite).

#### Storage Format

- **Key**: file path (UTF-8 bytes)
- **Value**: bincode-serialized struct:

```rust
struct CacheEntry {
    size: u64,
    mtime_secs: i64,
    mtime_nanos: u32,
    inode: u64,
    hash_algo: String,
    hash_value: String,
    cached_at: String,  // ISO 8601 timestamp
}
```

#### Cache Invalidation

A cached hash is considered valid only if **all** of the following still match the current file:
- `size`
- `mtime_secs` + `mtime_nanos`
- `inode`

If any of these differ, the cached entry is stale — the file is rehashed and the cache entry is updated.

#### Cache Lifecycle

- The cache directory `~/.rsdedup/` is created on first run if it doesn't exist.
- `--no-cache` disables reading/writing the cache for that run (useful for benchmarking or one-off scans).
- `--clear-cache` deletes `cache.db` and exits.
- Stale entries (files that no longer exist) are pruned periodically — on every run, entries older than 30 days with no cache hit are removed.

#### Why sled

- Embedded key-value store, no daemon, no setup
- Single-file database at `~/.rsdedup/cache.db`
- Fast indexed lookups by path
- Well-supported in Rust via `sled` crate
- No schema overhead — just serialize the cache entry as the value with the path as the key

### Parallelism

- Directory walking uses `walkdir` (single-threaded, I/O bound)
- File hashing uses `rayon` thread pool for parallel computation
- The number of worker threads is configurable via `--jobs`

## Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing |
| `walkdir` | Recursive directory traversal |
| `rayon` | Data parallelism for hashing |
| `sha2` | SHA-256 hashing |
| `xxhash-rust` | xxHash (fast non-crypto hash) |
| `blake3` | BLAKE3 hashing |
| `serde` + `serde_json` | JSON output |
| `csv` | CSV output |
| `sled` | Embedded key-value store for hash cache |
| `bincode` | Fast serialization for cache entries |
| `anyhow` | Error handling |
| `indicatif` | Progress bars |

## Safety

- **Default action is report-only** — no files are modified unless explicitly requested
- **`--dry-run`** shows what would happen without making changes
- **Delete action requires `--keep`** strategy to be explicit about which file survives
- **No cross-filesystem hardlinks** — detected and reported as errors
- **Symlink loops** are avoided by not following symlinks by default
