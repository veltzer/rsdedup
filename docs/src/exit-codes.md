# Exit Codes

rsdedup uses meaningful exit codes for scripting:

| Code | Meaning |
|------|---------|
| `0` | Success, no duplicates found |
| `1` | Success, duplicates found |
| `2` | Error |

## Examples

```bash
# Check if a directory has duplicates
if rsdedup dedup report /data > /dev/null 2>&1; then
    echo "No duplicates"
else
    echo "Duplicates found"
fi

# Use in CI to fail if duplicates exist
rsdedup dedup report /assets && echo "Clean" || echo "Duplicates detected"
```
