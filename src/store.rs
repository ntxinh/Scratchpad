use gtk4::glib;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize, Deserialize)]
pub struct PadMeta {
    pub id: String,
    pub name: String,
    pub repo_path: Option<PathBuf>,
    pub modified: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct Metadata {
    pads: HashMap<String, PadMeta>,
}

pub struct Store {
    pub dir: PathBuf,
}

impl Store {
    /// XDG data dir (`~/.local/share/scratchpad`), creates dirs.
    pub fn new() -> Self {
        Self::at(glib::user_data_dir().join("scratchpad"))
    }

    /// Test ctor pointing at an arbitrary dir.
    pub fn at(dir: PathBuf) -> Self {
        fs::create_dir_all(dir.join("scratchpads")).ok();
        Self { dir }
    }

    fn pad_path(&self, id: &str) -> PathBuf {
        self.dir.join("scratchpads").join(format!("{id}.md"))
    }

    fn meta_path(&self) -> PathBuf {
        self.dir.join("metadata.json")
    }

    pub fn load(&self, id: &str) -> String {
        fs::read_to_string(self.pad_path(id)).unwrap_or_default()
    }

    /// Write-tmp-then-rename so readers never see a torn file.
    pub fn save(&self, id: &str, text: &str) -> io::Result<()> {
        write_atomic(&self.pad_path(id), text.as_bytes())?;
        self.bump_modified(id);
        Ok(())
    }

    /// Every *.md in scratchpads/, newest first, merged with metadata.json.
    pub fn list(&self) -> Vec<PadMeta> {
        let meta = self.read_meta();
        let mut out = vec![];
        if let Ok(rd) = fs::read_dir(self.dir.join("scratchpads")) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "md") {
                    let id = p.file_stem().unwrap().to_string_lossy().into_owned();
                    out.push(meta.pads.get(&id).cloned().map_or_else(
                        || PadMeta {
                            name: id.clone(),
                            id,
                            repo_path: None,
                            modified: file_mtime(&p),
                        },
                        // metadata can lag behind out-of-band edits — file mtime wins
                        |m| PadMeta {
                            modified: m.modified.max(file_mtime(&p)),
                            ..m
                        },
                    ));
                }
            }
        }
        out.sort_by_key(|m| Reverse(m.modified));
        out
    }

    /// (id, 1-based line number, line text) for every line containing `term`.
    pub fn search(&self, term: &str) -> Vec<(String, usize, String)> {
        let mut hits = vec![];
        for m in self.list() {
            for (i, line) in self.load(&m.id).lines().enumerate() {
                if line.contains(term) {
                    hits.push((m.id.clone(), i + 1, line.to_string()));
                }
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
            id: id.into(),
            name: id.into(),
            repo_path: None,
            modified: now(),
        });
        e.repo_path = Some(repo.to_path_buf());
        self.write_meta(&meta);
    }

    fn bump_modified(&self, id: &str) {
        let mut meta = self.read_meta();
        meta.pads
            .entry(id.to_string())
            .or_insert_with(|| PadMeta {
                id: id.into(),
                name: id.into(),
                repo_path: None,
                modified: 0,
            })
            .modified = now();
        self.write_meta(&meta);
    }
    fn write_meta(&self, meta: &Metadata) {
        write_atomic(
            &self.meta_path(),
            serde_json::to_string(meta).unwrap().as_bytes(),
        )
        .ok();
    }

    fn read_meta(&self) -> Metadata {
        fs::read_to_string(self.meta_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

fn file_mtime(p: &Path) -> u64 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Writes to `path.tmp` then renames — readers never see a torn file.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    // appends ".tmp" (metadata.json → metadata.json.tmp, a.md → a.md.tmp)
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Unique dir per test so parallel runs don't collide.
    fn tmp() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        std::env::temp_dir().join(format!(
            "sp-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ))
    }

    /// Removes the test dir on drop.
    struct TmpDir(PathBuf);

    impl TmpDir {
        fn new() -> Self {
            Self(tmp())
        }
        fn path(&self) -> PathBuf {
            self.0.clone()
        }
    }

    impl Drop for TmpDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn save_then_load_roundtrips() {
        let t = TmpDir::new();
        let s = Store::at(t.path());
        s.save("default", "# hi\n").unwrap();
        assert_eq!(s.load("default"), "# hi\n");
    }

    #[test]
    fn load_missing_returns_empty() {
        let t = TmpDir::new();
        assert_eq!(Store::at(t.path()).load("nope"), "");
    }

    #[test]
    fn list_finds_pads_and_modified_is_nonzero() {
        let t = TmpDir::new();
        let s = Store::at(t.path());
        s.save("default", "a").unwrap();
        s.save("repo-abc", "b").unwrap();
        let ids: Vec<_> = s.list().iter().map(|m| m.id.clone()).collect();
        assert!(ids.contains(&"default".into()) && ids.contains(&"repo-abc".into()));
        assert!(s.list().iter().all(|m| m.modified > 0));
    }

    #[test]
    fn search_matches_lines_across_pads() {
        let t = TmpDir::new();
        let s = Store::at(t.path());
        s.save("a", "foo\nbar baz\n").unwrap();
        s.save("b", "nothing\nbaz too\n").unwrap();
        let hits = s.search("baz");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].1, 2); // line number, 1-based
    }

    #[test]
    fn export_writes_verbatim() {
        let t = TmpDir::new();
        let s = Store::at(t.path());
        s.save("a", "# note\n").unwrap();
        let dest = t.path().join("out.md");
        s.export("a", &dest).unwrap();
        assert_eq!(fs::read_to_string(&dest).unwrap(), "# note\n");
    }

    #[test]
    fn save_is_atomic_no_torn_file() {
        // observable proxy: after save, no *.tmp artifacts remain
        let t = TmpDir::new();
        let s = Store::at(t.path());
        s.save("a", "x").unwrap();
        assert_eq!(
            fs::read_dir(s.dir.join("scratchpads"))
                .unwrap()
                .filter(|e| e
                    .as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|x| x == "tmp"))
                .count(),
            0
        );
    }
}
