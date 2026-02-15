# tmxu

A tmux session browser TUI. Run one command, visually browse sessions/windows/panes, press Enter to attach.

## Installation

### From crates.io

```sh
cargo install tmxu
```

### Pre-built binaries

Download a pre-built binary from [GitHub Releases](https://github.com/adammeghji/tmxu/releases/latest):

| Platform | Archive |
|----------|---------|
| Linux x86_64 | `tmxu-*-x86_64-unknown-linux-gnu.tar.gz` |
| macOS Intel | `tmxu-*-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `tmxu-*-aarch64-apple-darwin.tar.gz` |

```sh
# Example: macOS Apple Silicon
tar xzf tmxu-*-aarch64-apple-darwin.tar.gz
sudo mv tmxu /usr/local/bin/
```

### Build from source

```sh
git clone https://github.com/adammeghji/tmxu.git
cd tmxu
cargo install --path .
```

## Usage

```sh
tmxu
tmxu --no-logo
```

## Keybindings

| Key | Action |
|-----|--------|
| `a`-`z` | Select session |
| `A`-`Z` | Open session (attach immediately) |
| `1`-`9` | Select window |
| `j`/`k` | Navigate |
| `Enter` | Attach to selected session/window |
| `Space`/`l` | Expand |
| `h` | Collapse |
| `n` | New session |
| `d` | Kill session |
| `r` | Rename session |
| `R` | Refresh |
| `g`/`G` | First/last |
| `q`/`Esc` | Quit |

## License

MIT
