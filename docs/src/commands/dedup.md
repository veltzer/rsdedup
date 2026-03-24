# dedup

Find and act on duplicate files.

```
rsdedup dedup <subcommand> [options] [path]
```

All dedup subcommands default to the current directory if no path is given.

## Subcommands

### report

Find and report duplicate files. No files are modified.

```bash
rsdedup dedup report
rsdedup dedup report /home/user/photos
rsdedup dedup report --output json /data
```

### delete

Delete duplicate files, keeping one copy per group.

```bash
rsdedup dedup delete /home/user/photos
rsdedup dedup delete --keep oldest /home/user/photos
rsdedup dedup delete --dry-run /home/user/photos
```

| Flag | Description | Default |
|------|-------------|---------|
| `--keep <STRATEGY>` | Which file to keep: `first`, `newest`, `oldest`, `shortest-path` | `first` |
| `-n, --dry-run` | Show what would be done without making changes | `false` |

#### Keep strategies

| Strategy | Description |
|----------|-------------|
| `first` | Keep the first file encountered during directory walk |
| `newest` | Keep the file with the most recent modification time |
| `oldest` | Keep the file with the oldest modification time |
| `shortest-path` | Keep the file with the shortest path |

### hardlink

Replace duplicate files with hardlinks to a single copy. All file paths continue to work, but they share the same disk blocks.

```bash
rsdedup dedup hardlink /data
rsdedup dedup hardlink --dry-run /data
```

| Flag | Description | Default |
|------|-------------|---------|
| `-n, --dry-run` | Show what would be done without making changes | `false` |

Hardlinks cannot cross filesystem boundaries. rsdedup will report an error if duplicates span different filesystems.

### symlink

Replace duplicate files with symbolic links to a single copy.

```bash
rsdedup dedup symlink /data
rsdedup dedup symlink --dry-run /data
```

| Flag | Description | Default |
|------|-------------|---------|
| `-n, --dry-run` | Show what would be done without making changes | `false` |
