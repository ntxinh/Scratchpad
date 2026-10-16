use std::path::{Path, PathBuf};

pub fn pad_id(cwd: &Path) -> String {
    repo_root(cwd)
        .map(|r| format!("repo-{:016x}", fnv1a(r.as_os_str().as_encoded_bytes())))
        .unwrap_or_else(|| "default".into())
}

pub fn repo_root(mut p: &Path) -> Option<PathBuf> {
    loop {
        if p.join(".git").exists() {
            return Some(p.to_path_buf());
        }
        p = p.parent()?;
    }
}

// ponytail: FNV-1a inline — DefaultHasher's output isn't stable across compiler versions
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Unique dir per test so parallel runs don't collide.
    fn tmp() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        std::env::temp_dir().join(format!(
            "sp-detect-test-{}-{}",
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
    fn inside_git_repo_gives_repo_id() {
        let t = TmpDir::new();
        let dir = t.path();
        fs::create_dir_all(dir.join("proj/.git")).unwrap();
        let nested = dir.join("proj/src/deep");
        fs::create_dir_all(&nested).unwrap();
        assert!(pad_id(&nested).starts_with("repo-"));
    }

    #[test]
    fn outside_git_gives_default() {
        let t = TmpDir::new();
        assert_eq!(pad_id(&t.path()), "default");
    }

    #[test]
    fn repo_id_is_stable_across_depths() {
        let t = TmpDir::new();
        let dir = t.path();
        fs::create_dir_all(dir.join("p/.git")).unwrap();
        fs::create_dir_all(dir.join("p/a/b")).unwrap();
        assert_eq!(pad_id(&dir.join("p")), pad_id(&dir.join("p/a/b")));
    }

    #[test]
    fn sibling_repos_differ() {
        let t = TmpDir::new();
        let dir = t.path();
        fs::create_dir_all(dir.join("a/.git")).unwrap();
        fs::create_dir_all(dir.join("b/.git")).unwrap();
        assert_ne!(pad_id(&dir.join("a")), pad_id(&dir.join("b")));
    }
}
