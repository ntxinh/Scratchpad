# Scratchpad Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Working `scratchpad` binary: niri keybind → `scratchpad toggle` → GTK4 layer-shell overlay → Markdown autosave to XDG files.

**Architecture:** Single binary, `GtkApplication` single-instance (second `toggle` D-Bus-activates the running one — activation IS the toggle). Headless layers `store`/`detect` are pure Rust + tests; `ui` is the only GTK zone, verified manually under niri.

**Tech Stack:** Rust stable (mise), gtk4 + libadwaita + gtk4-layer-shell-rs, pulldown-cmark (preview), serde/serde_json (metadata). No clap — 4 subcommands hand-parsed; no glib-extra dirs crate — `glib::user_data_dir()`.

**Spec:** `docs/specs.md`, `DESIGN.md`

## Global Constraints

- Rust via mise (`mise.toml` pins `rust = "stable"`); tasks run as `mise run <task>` / `make <task>`.
- Wayland/niri only — no X11 calls, no keygrab; niri owns the bind.
- Open path does zero heavy work: instance is resident; show = raise existing window.
- Storage: `$XDG_DATA_HOME/scratchpad/scratchpads/*.md` + `metadata.json`; `.md` files authoritative; writes are temp-then-rename.
- No network/database crates. Ever. Spec non-goal.
- `clippy -D warnings` and `cargo fmt --check` clean before every commit.
- No `gtksourceview` — plain `TextView` (syntax highlighting: add later if wanted).

## File map

```
src/main.rs    arg dispatch; CLI subcommands exit before GTK init
src/app.rs     GtkApplication id "dev.scratchpad.App", activate = toggle
src/store.rs   Store { dir } — load/save/list/search/export, metadata.json
src/detect.rs  association_for(cwd) -> Association { repo_path } -> pad id
src/ui/mod.rs  build_window(app, store) — layer-shell, editor, toolbar
src/ui/preview.rs  markdown -> TextBuffer with tags (read-only view)
```

All `pub` items named here are the contract — later tasks call them exactly.

---

### Task 1: Cargo scaffold + layer-shell window that opens

**Files:**
- Create: `Cargo.toml`, `src/main.rs`, `src/app.rs`, `src/ui/mod.rs`

**Interfaces:**
- Produces: `app::run() -> i32` (called from main when no CLI subcommand), `ui::build_window(app: &adw::Application)` — returns nothing yet, wires activation.

- [ ] **Step 1: Init + deps**

```bash
cargo init --name scratchpad
```

`Cargo.toml` (overwrite the generated `[dependencies]` block):

```toml
[package]
name = "scratchpad"
version = "0.1.0"
edition = "2021"

[dependencies]
gtk4 = "0.9"
libadwaita = { version = "0.7", features = ["v1_4"] }
gtk4-layer-shell = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
pulldown-cmark = "0.12"
```

- [ ] **Step 2: Minimal window**

`src/main.rs`:

```rust
mod app;
mod store;
mod detect;
mod ui;

fn main() -> i32 {
    app::run()
}
```

`src/app.rs`:

```rust
use adw::prelude::*;
use gtk4::{gio, glib};

pub const APP_ID: &str = "dev.scratchpad.App";

pub fn run() -> i32 {
    let application = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    application.connect_activate(|app| crate::ui::build_window(app));
    application.run().into()
}
```

`src/ui/mod.rs`:

```rust
use adw::prelude::*;
use gtk4::prelude::*;
use gtk4_layer_shell::{Edge, Layer, LayerShell};

pub fn build_window(app: &adw::Application) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Scratchpad")
        .default_width(560)
        .default_height(380)
        .build();

    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_anchor(Edge::Top, true);
    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::OnDemand);
    window.set_margin(Edge::Top, 80);

    let view = gtk4::TextView::builder().monospace(true).build();
    window.set_content(Some(&view));

    window.present();
}
```

- [ ] **Step 3: Smoke test under niri**

Run: `cargo build && cargo run`
Expected: floating overlay appears top-center; `cargo run` in a second terminal toggles via `present` (single window, exits when closed).
If layer-shell crate name/features differ on first build: fix imports per the crate's current docs; the contract (`init_layer_shell`, `set_layer`, `set_anchor`, `set_keyboard_mode`) is stable.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock src/
git commit -m "feat: layer-shell overlay window opens on activate"
```

---

### Task 2: `store.rs` — persistence with real tests

**Files:**
- Create: `src/store.rs`

**Interfaces:**
- Produces (exact — Tasks 4–7 call these):

```rust
pub struct Store { dir: PathBuf }
pub struct PadMeta { pub id: String, pub name: String,
                     pub repo_path: Option<PathBuf>, pub modified: u64 }
impl Store {
    pub fn new() -> Self                    // XDG data dir, creates dirs
    pub fn at(dir: PathBuf) -> Self         // test ctor, no mkdir
    pub fn load(&self, id: &str) -> String  // "" if missing
    pub fn save(&self, id: &str, text: &str) -> io::Result<()>  // atomic
    pub fn list(&self) -> Vec<PadMeta>      // scans *.md, merges metadata.json
    pub fn search(&self, term: &str) -> Vec<(String, usize, String)> // (id, line, text)
    pub fn export(&self, id: &str, dest: &Path) -> io::Result<()>
    pub fn set_repo(&self, id: &str, repo: &Path) // updates metadata.json
}
```

- [ ] **Step 1: Failing tests** — `#[cfg(test)] mod tests` in `src/store.rs` using `std::env::temp_dir().join(format!("sp-test-{}", std::process::id()))` per test, `fs::remove_dir_all` in a drop-guard:

```rust
#[test]
fn save_then_load_roundtrips() {
    let s = Store::at(tmp());
    s.save("default", "# hi\n").unwrap();
    assert_eq!(s.load("default"), "# hi\n");
}

#[test]
fn load_missing_returns_empty() {
    assert_eq!(Store::at(tmp()).load("nope"), "");
}

#[test]
fn list_finds_pads_and_modified_is_nonzero() {
    let s = Store::at(tmp());
    s.save("default", "a").unwrap();
    s.save("repo-abc", "b").unwrap();
    let ids: Vec<_> = s.list().iter().map(|m| m.id.clone()).collect();
    assert!(ids.contains(&"default".into()) && ids.contains(&"repo-abc".into()));
    assert!(s.list().iter().all(|m| m.modified > 0));
}

#[test]
fn search_matches_lines_across_pads() {
    let s = Store::at(tmp());
    s.save("a", "foo\nbar baz\n").unwrap();
    s.save("b", "nothing\nbaz too\n").unwrap();
    let hits = s.search("baz");
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].1, 2); // line number, 1-based
}

#[test]
fn export_writes_verbatim() {
    let s = Store::at(tmp());
    s.save("a", "# note\n").unwrap();
    let dest = tmp().join("out.md");
    s.export("a", &dest).unwrap();
    assert_eq!(fs::read_to_string(&dest).unwrap(), "# note\n");
}

#[test]
fn save_is_atomic_no_torn_file() {
    // observable proxy: after save, no *.tmp artifacts remain
    let s = Store::at(tmp());
    s.save("a", "x").unwrap();
    assert_eq!(fs::read_dir(s.dir.join("scratchpads")).unwrap()
        .filter(|e| e.as_ref().unwrap().path().extension().is_some_and(|x| x == "tmp"))
        .count(), 0);
}
```

- [ ] **Step 2: Verify fail** — `cargo test` → compile error (types don't exist).

- [ ] **Step 3: Implement** `src/store.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize)]
pub struct PadMeta {
    pub id: String,
    pub name: String,
    pub repo_path: Option<PathBuf>,
    pub modified: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct Metadata { pads: HashMap<String, PadMeta> }

pub struct Store { pub dir: PathBuf }

impl Store {
    pub fn new() -> Self {
        let s = Self::at(glib::user_data_dir().join("scratchpad"));
        fs::create_dir_all(s.dir.join("scratchpads")).ok();
        s
    }
    pub fn at(dir: PathBuf) -> Self {
        fs::create_dir_all(dir.join("scratchpads")).ok();
        Self { dir }
    }
    fn pad_path(&self, id: &str) -> PathBuf { self.dir.join("scratchpads").join(format!("{id}.md")) }
    fn meta_path(&self) -> PathBuf { self.dir.join("metadata.json") }

    pub fn load(&self, id: &str) -> String {
        fs::read_to_string(self.pad_path(id)).unwrap_or_default()
    }

    pub fn save(&self, id: &str, text: &str) -> io::Result<()> {
        let tmp = self.pad_path(id).with_extension("tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, self.pad_path(id))?;
        self.bump_modified(id);
        Ok(())
    }

    pub fn list(&self) -> Vec<PadMeta> {
        let meta = self.read_meta();
        let mut out = vec![];
        if let Ok(rd) = fs::read_dir(self.dir.join("scratchpads")) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "md") {
                    let id = p.file_stem().unwrap().to_string_lossy().into_owned();
                    out.push(meta.pads.get(&id).cloned().unwrap_or_else(|| PadMeta {
                        name: id.clone(), id, repo_path: None, modified: file_mtime(&p),
                    }));
                }
            }
        }
        out.sort_by(|a, b| b.modified.cmp(&a.modified));
        out
    }

    pub fn search(&self, term: &str) -> Vec<(String, usize, String)> {
        let mut hits = vec![];
        for m in self.list() {
            for (i, line) in self.load(&m.id).lines().enumerate() {
                if line.contains(term) { hits.push((m.id.clone(), i + 1, line.to_string())); }
            }
        }
        hits
    }

    pub fn export(&self, id: &str, dest: &Path) -> io::Result<()> {
        fs::copy(self.pad_path(id), dest)?;
        Ok(())
    }

    pub fn set_repo(&self, id: &str, repo: &Path) {
        let mut meta = self.read_meta();
        let e = meta.pads.entry(id.to_string()).or_insert_with(|| PadMeta {
            id: id.into(), name: id.into(), repo_path: None, modified: now(),
        });
        e.repo_path = Some(repo.to_path_buf());
        fs::write(self.meta_path(), serde_json::to_string(&meta).unwrap()).ok();
    }

    fn bump_modified(&self, id: &str) {
        let mut meta = self.read_meta();
        meta.pads.entry(id.to_string()).or_insert_with(|| PadMeta {
            id: id.into(), name: id.into(), repo_path: None, modified: 0,
        }).modified = now();
        fs::write(self.meta_path(), serde_json::to_string(&meta).unwrap()).ok();
    }

    fn read_meta(&self) -> Metadata {
        fs::read_to_string(self.meta_path())
            .ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
}

fn file_mtime(p: &Path) -> u64 {
    fs::metadata(p).and_then(|m| m.modified()).ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs()).unwrap_or(0)
}
fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() }
```

(Plus the test module + `fn tmp() -> PathBuf` helper from step 1.)

- [ ] **Step 4: `cargo test` → all pass.**

- [ ] **Step 5: Commit** — `git add src/store.rs && git commit -m "feat: markdown scratchpad store with atomic saves"`

---

### Task 3: `detect.rs` — repo association

**Files:**
- Create: `src/detect.rs`

**Interfaces:**
- Produces: `pub fn pad_id(cwd: &Path) -> String` — `"repo-<8-hex>"` inside a git worktree (stable hash of repo root), `"default"` otherwise. Task 5 calls it on every activate.

- [ ] **Step 1: Failing tests** in `src/detect.rs`:

```rust
#[test]
fn inside_git_repo_gives_repo_id() {
    let dir = tmp(); fs::create_dir_all(dir.join("proj/.git")).unwrap();
    let nested = dir.join("proj/src/deep"); fs::create_dir_all(&nested).unwrap();
    assert!(pad_id(&nested).starts_with("repo-"));
}

#[test]
fn outside_git_gives_default() {
    assert_eq!(pad_id(&tmp()), "default");
}

#[test]
fn repo_id_is_stable_across_depths() {
    let dir = tmp(); fs::create_dir_all(dir.join("p/.git")).unwrap();
    fs::create_dir_all(dir.join("p/a/b")).unwrap();
    assert_eq!(pad_id(&dir.join("p")), pad_id(&dir.join("p/a/b")));
}

#[test]
fn sibling_repos_differ() {
    let dir = tmp();
    fs::create_dir_all(dir.join("a/.git")).unwrap();
    fs::create_dir_all(dir.join("b/.git")).unwrap();
    assert_ne!(pad_id(&dir.join("a")), pad_id(&dir.join("b")));
}
```

- [ ] **Step 2: Verify fail.**

- [ ] **Step 3: Implement** — walk ancestors for `.git` (dir OR file — worktrees have a `.git` file), hash the root path with `std::collections::hash_map::DefaultHasher` (no dep; stable within a run is fine *only if* we need cross-run stability — we do: same repo → same pad across sessions. `DefaultHasher` is NOT guaranteed stable across Rust versions. Ponytail call: use a tiny inline FNV-1a, 10 lines, deterministic forever):

```rust
use std::hash::Hasher;
use std::path::{Path, PathBuf};

pub fn pad_id(cwd: &Path) -> String {
    repo_root(cwd).map(|r| format!("repo-{:016x}", fnv1a(r.as_os_str().as_encoded_bytes())))
        .unwrap_or_else(|| "default".into())
}

pub fn repo_root(mut p: &Path) -> Option<PathBuf> {
    loop {
        if p.join(".git").exists() { return Some(p.to_path_buf()); }
        p = p.parent()?;
    }
}

// ponytail: FNV-1a inline — DefaultHasher's output isn't stable across compiler versions
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in bytes { h ^= b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}
```

(truncated to `{:016x}` → 16 hex chars; collision odds across one dev's repos ≈ nil.)

- [ ] **Step 4: `cargo test` → pass.**

- [ ] **Step 5: Commit** — `git commit -m "feat: git-repo scratchpad association"`

---

### Task 4: CLI subcommands in `main.rs`

**Files:**
- Modify: `src/main.rs`
- Modify: `src/app.rs` (toggle semantics)

**Interfaces:**
- Consumes: `Store`, `detect::pad_id`.
- Produces: `scratchpad [toggle|list|export <id> <path>|search <term>]`. `toggle`/bare → GTK; others → headless, exit 0/1.

- [ ] **Step 1: Failing test** — put subcommand dispatch in `store`-testable form. In `src/main.rs` add `#[cfg(test)]` tests driving a `run_cli(args, &store) -> i32` fn:

```rust
#[test]
fn list_prints_pad_ids() { /* capture via run_cli writing to a String sink — return String instead */ }
```

Cleaner: `run_cli(args: &[String], store: &Store, out: &mut String) -> i32` — tests assert on `out`. `main` passes a real stdout buffer.

```rust
fn main() -> i32 {
    let args: Vec<String> = std::env::args().collect();
    let store = store::Store::new();
    match args.get(1).map(String::as_str) {
        None | Some("toggle") => app::run(),          // D-Bus activation handles the rest
        _ => { let mut out = String::new();
               let code = run_cli(&args[1..], &store, &mut out);
               print!("{out}"); code }
    }
}
```

Tests:

```rust
#[test]
fn search_cli_finds_line() {
    let s = Store::at(tmp()); s.save("a", "needles here\n").unwrap();
    let mut out = String::new();
    assert_eq!(run_cli(&["search".into(), "needles".into()], &s, &mut out), 0);
    assert!(out.contains("a:1") && out.contains("needles"));
}

#[test]
fn export_cli_copies() { /* save "a", run_cli export to tmp path, assert file exists */ }

#[test]
fn unknown_subcommand_is_usage_error() {
    let mut out = String::new();
    assert_eq!(run_cli(&["bogus".into()], &Store::at(tmp()), &mut out), 2);
}
```

- [ ] **Step 2: Verify fail.**

- [ ] **Step 3: Implement** `run_cli` — `list` prints `id\tname`, `search` prints `id:line: text`, `export` copies + prints `exported <id> -> <path>`, else usage to `out` + code 2. In `app.rs` activate handler becomes the toggle (Task 5 wires hide/show; for now `build_window` already `present`s — add: if window exists and is visible → `window.close()`; else build. Track via `app.active_window()`).

- [ ] **Step 4: `cargo test` → pass.**

- [ ] **Step 5: Commit** — `git commit -m "feat: list/export/search CLI subcommands"`

---

### Task 5: Editor + autosave + toggle show/hide (core UX)

**Files:**
- Modify: `src/app.rs`, `src/ui/mod.rs`

**Interfaces:**
- Consumes: `Store::new`, `detect::pad_id`, `std::env::current_dir`.
- Produces: activation flow `pad = pad_id(cwd); text = store.load(pad); show window; debounce 500ms → store.save; hide/close → store.save`. Later tasks consume `ui::state::current_pad()`.

- [ ] **Step 1: Implement** — in `app.rs` `connect_activate`:

```rust
application.connect_activate(|app| {
    if let Some(w) = app.active_window() {
        if w.is_visible() { crate::ui::hide_and_save(&w); return; }
        crate::ui::show_for_cwd(&w); return;
    }
    crate::ui::build_window(app);
});
```

In `src/ui/mod.rs` — module state (single window, single instance; `thread_local!`):

```rust
thread_local! {
    static STORE: Store = Store::new();
    static PAD: std::cell::RefCell<String> = RefCell::new("default".into());
    static SAVE_SRC: std::cell::RefCell<Option<glib::SourceId>> = RefCell::new(None);
}
```

`build_window`: layer-shell setup as Task 1 + a `TextView` whose buffer loads `store.load(&pad)`, `connect_changed` → debounce:

```rust
buffer.connect_changed(|buf| {
    SAVE_SRC.with(|s| {
        if let Some(id) = s.borrow_mut().take() { id.remove(); }
        let b = buf.clone();
        *s.borrow_mut() = Some(glib::timeout_add_local_once(
            std::time::Duration::from_millis(500), move || {
                STORE.with(|st| PAD.with(|p| {
                    st.save(&p.borrow(), &b.text(&b.start_iter(), &b.end_iter(), false)).ok();
                }));
            }));
    });
});
```

`hide_and_save(w)`: flush pending save (remove SourceId, save now), `w.set_visible(false)`.
`show_for_cwd(w)`: `pad = pad_id(&current_dir)`, load text into buffer, `w.present()`.
`connect_close_request` → `hide_and_save`-equivalent then `PROPAGATE` (or prevent default and hide — keep resident).

- [ ] **Step 2: Manual verify under niri** — bind per README; open, type, close (Mod+space), reopen: text persists. `ls ~/.local/share/scratchpad/scratchpads/` shows `default.md` (or `repo-*.md` from inside a repo). Check file content matches.

- [ ] **Step 3: Commit** — `git commit -m "feat: autosaving editor, toggle show/hide, repo association live"`

---

### Task 6: Markdown preview toggle

**Files:**
- Create: `src/ui/preview.rs`
- Modify: `src/ui/mod.rs`

**Interfaces:**
- Produces: `pub fn render_markdown(md: &str, buf: &gtk4::TextBuffer)` — applies tags `h1/h2/code/italic`; `Ctrl+P` or toolbar button swaps editor↔preview in a `gtk4::Stack`.

- [ ] **Step 1: Implement** `preview.rs` — create tag table once:

```rust
let tags = buf.tag_table();
// bold h1 (scale 1.6), h2 (1.3), code (monospace + bg via style class is fine: family monospace)
```

Then `pulldown_cmark::Parser::new(md)` → track active tags in a stack; on `Event::Text(t)` insert at end iter with current tag set; on `Start(Heading{level})` push `h{level}`; `End` pop; `CodeBlock` → push `code`. ~60 lines. Editor and preview share content via two buffers on a `Stack`: `stack.add_named(edit_view, "edit")`, `stack.add_named(preview_view, "preview")`; button/`Ctrl+P` re-renders preview from editor buffer then `set_visible_child_name`.

- [ ] **Step 2: Manual verify** — type `# Hi\n\nsome **code**`, Ctrl+P → styled render; Ctrl+P back → editable.

- [ ] **Step 3: Commit** — `git commit -m "feat: markdown preview"`

---

### Task 7: Toolbar — switcher, search, quick actions

**Files:**
- Modify: `src/ui/mod.rs`

**Interfaces:**
- Consumes: `Store::list`, `Store::search`, `ui` pad state.
- Produces: `adw::HeaderBar` with: pad dropdown (`adw::ComboRow`-equivalent as a `Popover` + `ListBox` — simpler than dropdown model), search `SearchEntry` filtering the list popover results, buttons: ```` ``` ```` fence, `- [ ]`, copy-all.

- [ ] **Step 1: Implement** — headerbar contents (left→right): pad button (title = current pad, click → popover listing `store.list()` + search entry; row-activate → save current, set PAD, load), spacer, three `Button`s:

```rust
// fence: insert "\n```\n\n```\n" around cursor; todo: "- [ ] "; copy-all: clipboard().set_text(&all_text)
```

- [ ] **Step 2: Manual verify** — create 2 pads (open in repo + ~), switch via popover, search filters, copy-all puts full text on clipboard (`wl-paste` to confirm).

- [ ] **Step 3: Commit** — `git commit -m "feat: pad switcher, search, quick actions"`

---

### Task 8: Install, bind, final polish

**Files:**
- Modify: `README.md` if any usage drifted

- [ ] **Step 1:** `mise run install` → `which scratchpad` resolves in `~/.local/bin`.

- [ ] **Step 2:** Add the niri bind (README kdl snippet) to `~/.config/niri/config.kdl`, `niri msg action reload-config` (or restart niri). Press bind: overlay <150 ms (it will — resident). Toggle off/on: content persists.

- [ ] **Step 3:** Cold-boot check: log out/in, press bind, content restored.

- [ ] **Step 4:** `mise run lint` clean; `git commit` any drift.

---

## Self-review notes

- Spec coverage: instant access ✓ (T1/T5/T8), markdown+preview+restore ✓ (T5/T6), multiple pads by repo/workspace ✓ (T3/T7; workspace tagging deliberately deferred — same metadata field, add when needed), content utilities ✓ (T7), search ✓ (T4 CLI + T7 UI), export ✓ (T4), local files only ✓ (T2), <150 ms ✓ (resident instance).
- Deliberate cuts (ponytail): no gtksourceview, no clap, no async I/O (debounce suffices), no workspace detection yet.
- Type consistency: `Store` methods, `pad_id(&Path) -> String`, `build_window(&adw::Application)`, `hide_and_save(&Window)`, `show_for_cwd(&Window)` used identically across tasks.
