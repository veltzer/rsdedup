# cache

Manage the persistent hash cache stored at `~/.rsdedup/cache.db`.

```
rsdedup cache <subcommand>
```

## Subcommands

### scan

Scan a directory and populate the hash cache with both partial (4KB) and full file hashes. No deduplication is performed.

```bash
rsdedup cache scan
rsdedup cache scan /home/user/photos
```

This is useful for warming up the cache before running dedup operations. On subsequent runs, unchanged files are skipped.

The scan command shows timing by default. Use `--no-timing` to suppress it.

```
cache location: /home/user/.rsdedup/cache.db
scanned 1234 files: 100 hashed, 1134 already cached
elapsed: 2.345s
```

### clear

Delete all entries from the hash cache.

```bash
rsdedup cache clear
```

### stats

Show detailed cache statistics.

```bash
rsdedup cache stats
```

Example output:

```
cache location:     /home/user/.rsdedup/cache.db
total entries:       1234
database size:       4.50 MB (4718592 bytes)
total file size:     12.34 GB (13249974886 bytes)
with partial hash:   1234
with full hash:      1234
stale (file gone):   3
oldest entry:        5d ago
newest entry:        2m ago
hash algorithms:
  sha256: 1234
```

### list

List all cache entries in tab-separated format, suitable for parsing with `awk`, `cut`, or other tools.

```bash
rsdedup cache list
```

Output columns:

| Column | Description |
|--------|-------------|
| `path` | File path |
| `size` | File size in bytes |
| `algo` | Hash algorithm used |
| `partial_hash` | Partial hash (first 4KB), empty if not computed |
| `full_hash` | Full file hash, empty if not computed |
| `cached_at` | Unix timestamp when the entry was cached |

Example:

```bash
# List all cached files
rsdedup cache list

# Find entries for a specific directory
rsdedup cache list | awk -F'\t' '$1 ~ /photos/'

# Show only files with full hashes
rsdedup cache list | awk -F'\t' '$5 != ""'
```
