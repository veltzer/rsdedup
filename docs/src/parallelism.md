# Parallelism

rsdedup uses multi-threading to speed up the comparison phase of duplicate detection.

## How it works

The comparison pipeline processes files grouped by size. Each size group is processed independently, which makes it a natural fit for parallelism. rsdedup uses [rayon](https://docs.rs/rayon/) to distribute size groups across a thread pool — multiple size groups are compared concurrently.

```
Size group A (all 1 KB files)  ──→  Thread 1
Size group B (all 5 KB files)  ──→  Thread 2
Size group C (all 12 KB files) ──→  Thread 3
...
```

### What is parallelized

- **Comparison phase** — size groups are processed in parallel using rayon's `par_iter`. Each thread handles hashing and comparing files within one size group.

### What is not parallelized

- **Directory scanning** — uses `walkdir` which is single-threaded and I/O-bound.
- **Actions** (delete, hardlink, symlink) — performed sequentially after duplicates are found.
- **Within a single size group** — files in the same size group are hashed and compared sequentially. This means a single large size group (many files of the same size) will not benefit from additional threads.

## Controlling thread count

Use the `--jobs` (or `-j`) flag to set the number of worker threads:

```bash
# Use 4 threads
rsdedup dedup report --jobs 4 /data

# Use a single thread (no parallelism)
rsdedup dedup report --jobs 1 /data
```

The default is the number of CPU cores reported by `std::thread::available_parallelism()`.

## When parallelism helps most

Parallelism provides the biggest speedup when:

- There are **many size groups** with duplicates to compare — more groups means more work to distribute across threads.
- Files are **large** — hashing large files is CPU-intensive, so parallel hashing of different size groups gives a significant speedup.
- The storage is **fast** (SSD/NVMe) — on slow spinning disks, I/O is the bottleneck and adding threads may not help.

Parallelism helps less when:

- Most files fall into **one or a few size groups** — there isn't enough independent work to distribute.
- Files are **very small** — hashing is fast and the overhead of thread coordination dominates.
- Using `--compare byte-for-byte` — byte-for-byte comparison is I/O-heavy, so additional CPU threads offer less benefit.
