# Hash Algorithms

rsdedup supports three hash algorithms. Choose with `--hash <ALGO>`.

## SHA-256 (default)

A widely-used cryptographic hash function producing 256-bit digests. Very low collision probability.

```bash
rsdedup dedup report --hash sha256
```

## xxHash (xxh3-128)

A non-cryptographic hash optimized for speed. Produces 128-bit digests. Significantly faster than SHA-256 for large files.

```bash
rsdedup dedup report --hash xxhash
```

Use xxHash when you're scanning large datasets and trust that the files are not adversarially crafted.

## BLAKE3

A modern cryptographic hash that's both fast and secure. Often faster than SHA-256 while providing equivalent security.

```bash
rsdedup dedup report --hash blake3
```

## Comparison

| Algorithm | Type | Output | Speed | Security |
|-----------|------|--------|-------|----------|
| SHA-256 | Cryptographic | 256-bit | Moderate | High |
| xxHash | Non-cryptographic | 128-bit | Very fast | None |
| BLAKE3 | Cryptographic | 256-bit | Fast | High |

For most users, the default SHA-256 is fine. If performance matters more than cryptographic guarantees, use xxHash. If you want both speed and security, use BLAKE3.
