# Hash Cache

rsdedup maintains a persistent hash cache at `~/.rsdedup/cache.db` to avoid rehashing files that haven't changed.

## How it works

The cache is a key-value store (using [sled](https://crates.io/crates/sled)) where:

- **Key**: absolute file path
- **Value**: cached metadata and hash values

Each cache entry stores:

- File size
- Modification time (seconds + nanoseconds)
- Inode number
- Hash algorithm used
- Partial hash (first 4KB)
- Full file hash
- Timestamp of when the entry was cached

## Cache invalidation

A cached hash is considered valid only if **all** of the following still match the current file:

- Size
- Modification time (mtime)
- Inode number

If any of these differ, the file is rehashed and the cache entry is updated.

## Cache operations

```bash
# Pre-populate the cache
rsdedup cache scan /path/to/directory

# View cache statistics
rsdedup cache stats

# Clear the cache
rsdedup cache clear
```

## Disabling the cache

Use `--no-cache` to skip the cache entirely for a single run:

```bash
rsdedup dedup report --no-cache /path
```

This is useful for benchmarking or when you suspect cache corruption.

## Cache location

The cache is stored at `~/.rsdedup/cache.db`. The directory is created automatically on first use.

## Incremental scanning

The `cache scan` command is incremental. On repeated runs, only files that have changed (or are new) are hashed. Files that haven't changed are skipped. Both partial (4KB) and full hashes are stored for every file.
