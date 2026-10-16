# Scratchpad — agent guide

GTK4 layer-shell overlay for quick Markdown notes. Rust, single binary named `scratchpad`.

## Commands

- Build: `mise run build` (or `make build`)
- Test: `mise run test` (or `make test`)
- Lint: `mise run lint` (or `make lint`) — `cargo fmt --check` + `clippy -D warnings`
- Run: `mise run run`
- Install: `mise run install` → `~/.local/bin/scratchpad`

Run `lint` before considering any change done.

## Layering (intended — keep it)

```
main.rs          arg parsing, subcommand dispatch
app.rs           GtkApplication, single-instance, toggle activation
ui/              layer-shell window, markdown preview — GTK-only zone
store.rs         scratchpad files, metadata.json, autosave
detect.rs        $PWD → git repo/workspace association
```

- GTK types never cross out of `ui/`. `store`/`detect` are pure Rust, testable headless.
- No new dependencies without a reason; no network or database crates ever (spec non-goal).

## Hard constraints (from docs/specs.md)

- Open < 150 ms — no blocking I/O or heavy init on the show path. Autosave/async write is where durability lives.
- Never lose content — save on edit-debounce AND on hide/quit.
- Wayland only: no X11 assumptions, no global keygrab — niri spawns `scratchpad toggle`, D-Bus activation forwards it to the running instance.
- Layer-shell surface for the overlay; libadwaita for theming.
- Storage is plain Markdown under `$XDG_DATA_HOME/scratchpad/` + `metadata.json`. Markdown files are authoritative.

## Conventions

- rustfmt defaults, clippy clean at `-D warnings`.
- Tests next to code (`#[cfg(test)] mod tests`) for `store`/`detect`; UI logic verified manually under niri.
- Docs to keep in sync: `README.md` (usage/deps), `DESIGN.md` (architecture), `docs/specs.md` (product contract — change only on purpose).
