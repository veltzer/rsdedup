# Output Formats

rsdedup supports two output formats, selected with `--output <FORMAT>`.

## Text (default)

Human-readable output showing duplicate groups and a summary.

```
Group 1 — 3 files, 12 bytes each (hash: a948904f2f0f479b):
  /home/user/photos/img001.jpg
  /home/user/photos/backup/img001.jpg
  /home/user/photos/old/img001.jpg

--- Summary ---
Files scanned:    150
Duplicate groups: 1
Duplicate files:  2
Wasted space:     24 bytes
Action:           report
Files affected:   0
Space recovered:  0 bytes
```

## JSON

Machine-readable JSON output for scripting and integration with other tools.

```bash
rsdedup dedup report --output json /path
```

The duplicate groups are output as a JSON array:

```json
[
  {
    "group": 1,
    "size": 12,
    "hash": "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447",
    "files": [
      "/home/user/photos/img001.jpg",
      "/home/user/photos/backup/img001.jpg",
      "/home/user/photos/old/img001.jpg"
    ]
  }
]
```

Followed by a JSON summary object:

```json
{
  "files_scanned": 150,
  "duplicate_groups": 1,
  "duplicate_files": 2,
  "wasted_bytes": 24,
  "action_taken": "report",
  "files_affected": 0,
  "bytes_recovered": 0
}
```
