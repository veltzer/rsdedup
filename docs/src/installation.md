# Installation

## From crates.io

```bash
cargo install rsdedup
```

## From source

```bash
git clone https://github.com/veltzer/rsdedup.git
cd rsdedup
cargo install --path .
```

## Pre-built binaries

Download pre-built binaries for Linux (x86_64, aarch64), macOS (x86_64, aarch64), and Windows (x86_64) from the [GitHub Releases](https://github.com/veltzer/rsdedup/releases) page.

## Shell completions

After installing, generate shell completions:

```bash
# Bash
rsdedup completions bash > ~/.local/share/bash-completion/completions/rsdedup

# Zsh
rsdedup completions zsh > ~/.zfunc/_rsdedup

# Fish
rsdedup completions fish > ~/.config/fish/completions/rsdedup.fish
```
