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
