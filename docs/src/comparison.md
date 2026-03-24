# Comparison Strategies

rsdedup supports three strategies for determining whether files are duplicates. Choose with `--compare <METHOD>`.

## size-hash (default)

The default strategy uses a multi-stage pipeline for best performance:

1. **Size grouping** — files with unique sizes are immediately excluded (they can't be duplicates)
2. **Partial hash** — hash only the first 4KB of each file; files with unique partial hashes are excluded
3. **Full hash** — hash the entire file for remaining candidates

This avoids reading entire files when a quick check can rule out matches. For most workloads, the vast majority of files are eliminated in stages 1 and 2.

```bash
rsdedup dedup report --compare size-hash  # default, same as omitting
```

## hash

Skip the partial hash stage and compute the full hash for all files in each size group.

This is simpler but slower for large files where the first 4KB would have been enough to distinguish them.

```bash
rsdedup dedup report --compare hash
```

## byte-for-byte

Compare files byte-by-byte without hashing. This guarantees zero false positives (no hash collisions possible) but is slower because every pair of candidate files must be read and compared.

```bash
rsdedup dedup report --compare byte-for-byte
```

## Which should I use?

| Strategy | Speed | False positives | Best for |
|----------|-------|-----------------|----------|
| `size-hash` | Fastest | Theoretically possible (cryptographic hash) | General use |
| `hash` | Fast | Theoretically possible | When files differ early and late but not in the first 4KB |
| `byte-for-byte` | Slowest | Zero | When absolute certainty is required |

For virtually all practical use cases, `size-hash` is the right choice.
