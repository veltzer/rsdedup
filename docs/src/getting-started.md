# Getting Started

## Find duplicates

The simplest way to use rsdedup is to report duplicates in the current directory:

```bash
rsdedup dedup report
```

Or specify a path:

```bash
rsdedup dedup report /home/user/photos
```

## Warm up the cache

For large directories, pre-populate the hash cache first. This makes subsequent operations much faster:

```bash
rsdedup cache scan /home/user/photos
```

## Preview before acting

Always use `--dry-run` before destructive operations:

```bash
# See what would be deleted
rsdedup dedup delete --dry-run /home/user/photos

# See what would be hardlinked
rsdedup dedup hardlink --dry-run /home/user/photos
```

## Delete duplicates

Delete duplicates, keeping the oldest file in each group:

```bash
rsdedup dedup delete --keep oldest /home/user/photos
```

## Save space with hardlinks

Replace duplicates with hardlinks — all copies still appear as separate files but share disk space:

```bash
rsdedup dedup hardlink /home/user/photos
```

## JSON output for scripting

```bash
rsdedup dedup report --output json /home/user/photos
```

## Typical workflow

```bash
# 1. Warm cache (optional, speeds up repeated runs)
rsdedup cache scan ~/photos

# 2. See what's duplicated
rsdedup dedup report ~/photos

# 3. Preview cleanup
rsdedup dedup delete --dry-run --keep oldest ~/photos

# 4. Execute
rsdedup dedup delete --keep oldest ~/photos
```
