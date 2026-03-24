# Design

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

## Parallelism

- Directory walking is single-threaded (I/O bound, using `walkdir`)
- File hashing uses a `rayon` thread pool for parallel computation
- Thread count is configurable via `--jobs`

## Cache Design

The hash cache uses `sled`, an embedded key-value store. Each entry maps a file path to its metadata (size, mtime, inode) and hash values (partial and full). Cache entries are invalidated when any metadata field changes. The cache merges partial and full hashes — computing one doesn't overwrite the other.

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
