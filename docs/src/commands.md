# Commands

rsdedup uses a two-level subcommand structure:

```
rsdedup <command> <subcommand> [options] [path]
```

## Top-level commands

| Command | Description |
|---------|-------------|
| `dedup` | Find and act on duplicate files |
| `cache` | Manage the hash cache |
| `version` | Show version and build information |
| `completions` | Generate shell completions |

## Global options

These options apply to all commands that scan files. They are hidden from the short help (`-h`) but visible in the long help (`--help`).

| Flag | Description | Default |
|------|-------------|---------|
| `--compare <METHOD>` | Comparison method: `size-hash`, `hash`, `byte-for-byte` | `size-hash` |
| `--hash <ALGO>` | Hash algorithm: `sha256`, `xxhash`, `blake3` | `sha256` |
| `--min-size <BYTES>` | Minimum file size to consider | none |
| `--max-size <BYTES>` | Maximum file size to consider | none |
| `-r, --recursive` | Recurse into subdirectories | `true` |
| `--no-recursive` | Do not recurse | `false` |
| `--follow-symlinks` | Follow symbolic links | `false` |
| `-v, --verbose` | Verbose output | `false` |
| `--output <FORMAT>` | Output format: `text`, `json` | `text` |
| `-j, --jobs <N>` | Number of parallel workers | CPU count |
| `--no-cache` | Disable the hash cache | `false` |
| `--no-timing` | Disable timing output | `false` |
| `--exclude <GLOB>` | Exclude files matching pattern (repeatable) | none |
| `--include <GLOB>` | Only include files matching pattern (repeatable) | none |
