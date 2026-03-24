# Introduction

**rsdedup** is a fast, Rust-based file deduplication tool. It scans directories for duplicate files and supports multiple actions: reporting, deleting, hardlinking, and symlinking duplicates.

## Key Features

- **Multiple actions** — report, delete, hardlink, or symlink duplicates
- **Smart comparison** — size grouping, then partial 4KB hash, then full hash
- **Multiple hash algorithms** — SHA-256, xxHash, BLAKE3
- **Persistent hash cache** — avoids rehashing unchanged files across runs
- **Parallel hashing** — configurable thread count for fast scanning
- **Flexible filtering** — include/exclude globs, min/max file size
- **Multiple output formats** — human-readable text or JSON
- **Dry-run mode** — preview destructive operations before executing
- **Shell completions** — bash, zsh, and fish

## Philosophy

rsdedup is designed to be:

- **Safe by default** — read-only operations unless you explicitly ask for changes
- **Fast** — multi-stage pipeline eliminates candidates early, parallel hashing
- **Incremental** — persistent cache means repeated scans are nearly instant
- **Unix-friendly** — composable with other tools via JSON output and meaningful exit codes
