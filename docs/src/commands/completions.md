# completions

Generate shell completion scripts.

```bash
rsdedup completions <shell>
```

Supported shells: `bash`, `zsh`, `fish`, `elvish`, `powershell`.

## Examples

```bash
# Bash
rsdedup completions bash > ~/.local/share/bash-completion/completions/rsdedup

# Zsh (add ~/.zfunc to your fpath)
rsdedup completions zsh > ~/.zfunc/_rsdedup

# Fish
rsdedup completions fish > ~/.config/fish/completions/rsdedup.fish
```
