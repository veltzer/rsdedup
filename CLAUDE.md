# rsdedup

A fast Rust CLI tool for file deduplication.

## Build & Run

```bash
cargo build
cargo run -- <subcommand> [options]
cargo test
cargo clippy
cargo fmt -- --check
```

## Architecture

Pipeline-based: scan → group by size → filter → compare (hash) → act.

See `DESIGN.md` for the full design document.

### Module Layout

- `src/main.rs` — CLI parsing (clap) and orchestration
- `src/scanner.rs` — Directory walking with `walkdir`
- `src/grouper.rs` — Group files by size
- `src/compare.rs` — Comparison strategies (size-hash, hash, byte-for-byte)
- `src/hasher.rs` — Hash implementations (SHA256, xxHash, BLAKE3)
- `src/cache.rs` — Persistent hash cache using `sled` at `~/.rsdedup/cache.db`
- `src/action.rs` — Actions: report, hardlink, symlink, delete
- `src/types.rs` — Shared types (FileEntry, DuplicateGroup, etc.)
- `src/output.rs` — Output formatting (text, JSON, CSV)
- `src/error.rs` — Error types

### Subcommands

`report`, `delete`, `hardlink`, `symlink`, `scan`, `cache`

All scan subcommands default to the current directory.

## Code Style

- Use `cargo fmt` formatting
- Use `cargo clippy` with no warnings
- Use `anyhow` for error handling (`Result<T>` = `anyhow::Result<T>`)
- Prefer `thiserror` for library-style error enums if needed
- Keep modules focused — one responsibility per file
