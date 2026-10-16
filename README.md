# Scratchpad

Instant-access Markdown scratchpad for quick notes while programming. Opens via
niri keybind, auto-saves, keeps multiple scratchpads per project/repo. Local
files only — no database, no network, no sync.

## Requirements

- Fedora Workstation 44, Wayland, niri
- [mise](https://mise.jdrh.dev)
- System packages:

```sh
sudo dnf install gtk4-devel libadwaita-devel gtk4-layer-shell-devel
```

## Setup

```sh
mise install        # rust toolchain
mise run install    # installs `scratchpad` to ~/.local/bin
```

## Global shortcut

niri `config.kdl` — `scratchpad toggle` shows/hides the running instance:

```kdl
binds {
    Mod+space { spawn "scratchpad" "toggle"; }
}
```

The compositor owns the keybind; the app can't grab keys under Wayland.

## Usage

| Command | Action |
|---|---|
| `scratchpad` | Open the overlay (foreground) |
| `scratchpad toggle` | Show/hide via D-Bus activation of the running instance |
| `scratchpad list` | List scratchpads |
| `scratchpad search <term>` | Search across scratchpads (`id:line: text`) |
| `scratchpad export <id> <path>` | Promote a scratchpad to a `.md` file on disk |

Scratchpad association is automatic: if `$PWD` is inside a git repo, notes land
on that repo's scratchpad; otherwise a fallback scratchpad is used.

## Storage

All state lives in `$XDG_DATA_HOME/scratchpad/` (usually `~/.local/share/scratchpad/`):

```
scratchpad/
├── scratchpads/
│   ├── default.md          # fallback, no association
│   └── repo-<hash>.md      # one per repo/project
└── metadata.json           # id → {name, repo_path, workspace, created, modified}
```

Markdown files are the source of truth; `metadata.json` is index material only.

## Development

| Command | Action |
|---|---|
| `mise run build` / `make build` | Debug build |
| `mise run test` / `make test` | Tests |
| `mise run lint` / `make lint` | `fmt --check` + `clippy -D warnings` |
| `mise run install` / `make install` | Install to `~/.local/bin` |

Docs: [docs/specs.md](docs/specs.md) (product), [DESIGN.md](DESIGN.md) (architecture).
