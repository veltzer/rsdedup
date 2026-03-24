# Filtering

rsdedup provides several ways to control which files are considered.

## Include / Exclude globs

Use `--include` and `--exclude` to filter files by glob pattern. Both flags can be repeated.

```bash
# Only scan image files
rsdedup dedup report --include '*.jpg' --include '*.png'

# Skip log files and git directories
rsdedup dedup report --exclude '*.log' --exclude '.git/**'
```

Patterns are matched against both the filename and the full path.

When `--include` is specified, only files matching at least one include pattern are considered. When `--exclude` is specified, files matching any exclude pattern are skipped. If both are specified, exclude takes priority.

## File size filters

```bash
# Only consider files larger than 1MB
rsdedup dedup report --min-size 1048576

# Only consider files smaller than 100MB
rsdedup dedup report --max-size 104857600

# Combine both
rsdedup dedup report --min-size 1024 --max-size 104857600
```

## Recursion

By default, rsdedup recurses into subdirectories. Use `--no-recursive` to scan only the top-level directory:

```bash
rsdedup dedup report --no-recursive /data
```

## Symbolic links

By default, symbolic links are not followed. Use `--follow-symlinks` to follow them:

```bash
rsdedup dedup report --follow-symlinks /data
```
