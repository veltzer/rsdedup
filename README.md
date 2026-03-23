# rsdedup

A fast, Rust-based file deduplication tool.

## Features

- Multiple actions: report, delete, hardlink, symlink
- Comparison strategies: size+hash (default), hash-only, byte-for-byte
- Hash algorithms: SHA-256 (default), xxHash, BLAKE3
- Persistent hash cache (`~/.rsdedup/cache.db`) for fast repeated scans
- Parallel hashing with configurable thread count
- Include/exclude glob filters, min/max size filters
- JSON and text output formats
- Dry-run mode for destructive operations
- Shell completions for bash, zsh, and fish

## Installation

### From source

```bash
cargo install rsdedup
```

### From releases

Download a prebuilt binary from [GitHub Releases](https://github.com/veltzer/rsdedup/releases).

## Usage

```bash
# Report duplicates in current directory
rsdedup report

# Report duplicates in a specific directory
rsdedup report /home/user/photos

# Delete duplicates, keeping the oldest file
rsdedup delete --keep oldest /home/user/photos

# Hardlink duplicates (dry-run first)
rsdedup hardlink --dry-run /data

# Symlink duplicates
rsdedup symlink /data

# Use BLAKE3 with byte-for-byte comparison and JSON output
rsdedup report --hash blake3 --compare byte-for-byte --output json

# Only find duplicate images, excluding thumbnails
rsdedup report --include '*.jpg' --include '*.png' --exclude '*thumb*'

# Filter by file size
rsdedup report --min-size 1024 --max-size 100000000

# Pre-populate the hash cache
rsdedup scan /home/user/photos

# Show cache statistics
rsdedup cache stats

# Clear the cache
rsdedup cache clear

# Generate shell completions
rsdedup completions bash > ~/.local/share/bash-completion/completions/rsdedup

# Show version and build info
rsdedup version
```

## Subcommands

| Command | Description |
|---------|-------------|
| `report` | Find and list duplicate files (read-only) |
| `delete` | Remove duplicate files, keeping one per group |
| `hardlink` | Replace duplicates with hardlinks |
| `symlink` | Replace duplicates with symlinks |
| `scan` | Populate the hash cache without taking action |
| `cache` | Manage the hash cache (`clear`, `stats`) |
| `completions` | Generate shell completions |
| `version` | Show version and build information |

## How It Works

rsdedup uses a multi-stage pipeline to efficiently find duplicates:

1. **Scan** — Walk the directory tree and collect file metadata
2. **Group** — Group files by size (unique sizes are skipped)
3. **Filter** — Apply size and glob filters
4. **Compare** — Compare candidates using the chosen strategy
5. **Act** — Perform the requested action on duplicate groups

The default `size-hash` comparison strategy is optimized for speed:
files are first grouped by size, then a partial 4KB hash eliminates
most non-duplicates, and only remaining candidates get a full hash.

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | No duplicates found |
| `1` | Duplicates found |
| `2` | Error |

## License

MIT
