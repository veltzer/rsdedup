# complete

Generate shell completion scripts.

```bash
rsdedup complete <shell>
```

Supported shells: `bash`, `zsh`, `fish`, `elvish`, `powershell`.

## Examples

```bash
# Bash
rsdedup complete bash > ~/.local/share/bash-completion/complete/rsdedup

# Zsh (add ~/.zfunc to your fpath)
rsdedup complete zsh > ~/.zfunc/_rsdedup

# Fish
rsdedup complete fish > ~/.config/fish/complete/rsdedup.fish
```
