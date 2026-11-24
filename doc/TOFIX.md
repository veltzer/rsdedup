# TOFIX

Findings from a code scan on 2026-10-04.

## High

- `src/compare.rs:89,108,139` - a file that fails to hash (permission denied, vanished, I/O error) gets `unwrap_or_default()`, i.e. the empty string, as its hash; every unreadable file of the same size then lands in the same `""` bucket and is reported as a duplicate group, so `dedup delete --keep first` / `hardlink` / `symlink` can destroy files whose contents were never compared. Skip (and warn about) entries whose hash fails instead of keying them on `""`.
- `src/action.rs:93-123` - interactive `delete` (the default `--keep`, `src/cli.rs:150`) loops forever when stdin hits EOF: `read_line` returns `Ok(0)`, the empty input is "invalid", and the prompt repeats endlessly (e.g. `rsdedup dedup delete < /dev/null`, cron, CI). Treat `Ok(0)` as an error/abort, and refuse interactive mode when stdin is not a terminal.
- `README.md:33-66` - every usage example uses the old top-level commands (`rsdedup report|delete|hardlink|symlink|scan|completions`); the CLI now has `rsdedup dedup <action>`, `rsdedup cache scan` and `rsdedup complete` (`src/cli.rs:115-191`), so all of them fail. The subcommand table at `README.md:74-83` is wrong the same way; update both.

## Medium

- `src/compare.rs:35` - `.filter_map(|r| r.ok())` silently drops a whole size group when any comparison errors (e.g. `files_equal` failing to open one file in byte-for-byte mode, `compare.rs:159`); report the error and continue with the readable files instead of hiding the group.
- `src/hasher.rs:34-38` - full xxHash reads the entire file into memory with `read_to_end`, so hashing a multi-GB file can exhaust RAM; use the streaming `xxhash_rust::xxh3::Xxh3` hasher with the same 64 KiB buffer loop as SHA-256/BLAKE3.
- `src/action.rs:164-165` - `hardlink` (and `symlink`, `action.rs:205-206`) deletes the duplicate before creating the link; if `hard_link`/`symlink` fails the path is gone. Create the link under a temporary name in the same directory and `rename` it over the duplicate.
- `src/scanner.rs:40-116` - files that are already hardlinks of each other (same `dev`+`ino`) are scanned as separate files, so `report` counts them as wasted bytes and `delete` claims to "recover" space it does not free; collapse entries by `(dev, ino)` before grouping.
- `docs/src/parallelism.md:38` - says the default worker count is the number of CPU cores, but `--jobs` defaults to `1` (`src/cli.rs:68`); fix the doc or the default.
- `docs/src/cache.md:3,7,57` - the cache is documented as a sled database at `~/.rsdedup/cache.db`, but the code uses redb at `~/.rsdedup/cache.redb` (`src/cache.rs:43-46`). Same stale text in `docs/src/commands/cache.md:3,25,49`, `docs/src/design.md:37,84,116`, `README.md:10` and `CLAUDE.md:28`.
- `docs/src/installation.md:27-33` - completion examples use `rsdedup completions <shell>`; the subcommand is `complete` (`src/cli.rs:129`). `docs/src/design.md:103` also still shows `rsdedup report`.

## Low

- `src/cli.rs:33-40` - `-r/--recursive` is a bool flag with `default_value_t = true`, so passing it can never change anything; drop it and keep only `--no-recursive`.
- `src/compare.rs:184-186` - `files_equal` compares the result of two independent `read()` calls, which may legally return different short counts for identical files, producing a false "different"; use `read_exact`-style filling (or compare after filling both buffers fully).
- `src/cache.rs:34` - with `HOME` unset the cache silently goes to `./.rsdedup` in the current directory; fail with an error instead of scattering cache dirs.
- `src/cache.rs:239-243` - `flush()` is a documented no-op kept from the sled era, still called from `src/main.rs:104,303`; remove it and its callers.
- `PROBLEMS.txt:1` - empty tracked file; delete it.
- `CLAUDE.md:36` - subcommand list (`report`, `delete`, `hardlink`, `symlink`, `scan`, ...) is the pre-`dedup` CLI; update it to match `src/cli.rs`.
