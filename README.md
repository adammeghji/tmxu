# tmxu

A tmux session browser TUI. Run one command, visually browse sessions/windows/panes, press Enter to attach.

## Install

```sh
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
