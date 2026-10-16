mod app;
mod detect;
mod store;
mod ui;

use std::fmt::Write;
use std::path::Path;

fn usage(out: &mut String) {
    out.push_str("usage: scratchpad [toggle|list|search <term>|export <id> <path>]\n");
}

fn run_cli(args: &[String], store: &store::Store, out: &mut String) -> i32 {
    match args.first().map(String::as_str) {
        Some("list") => {
            for m in store.list() {
                writeln!(out, "{}\t{}", m.id, m.name).unwrap();
            }
            0
        }
        Some("search") => {
            let Some(term) = args.get(1) else {
                usage(out);
                return 2;
            };
            for (id, line, text) in store.search(term) {
                writeln!(out, "{id}:{line}: {text}").unwrap();
            }
            0
        }
        Some("export") => {
            let (Some(id), Some(dest)) = (args.get(1), args.get(2)) else {
                usage(out);
                return 2;
            };
            match store.export(id, Path::new(dest)) {
                Ok(()) => {
                    writeln!(out, "exported {id} -> {dest}").unwrap();
                    0
                }
                Err(e) => {
                    writeln!(out, "export {id}: {e}").unwrap();
                    1
                }
            }
        }
        _ => {
            usage(out);
            2
        }
    }
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = match args.get(1).map(String::as_str) {
        // D-Bus activation handles the toggle from here.
        None | Some("toggle") => app::run(),
        _ => {
            let store = store::Store::new();
            let mut out = String::new();
            let code = run_cli(&args[1..], &store, &mut out);
            print!("{out}");
            code
        }
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use store::Store;

    /// Unique dir per test so parallel runs don't collide.
    fn tmp() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        std::env::temp_dir().join(format!(
            "sp-cli-test-{}-{}",
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
    fn list_prints_id_tab_name() {
        let t = TmpDir::new();
        let s = Store::at(t.path());
        s.save("a", "x").unwrap();
        let mut out = String::new();
        assert_eq!(run_cli(&["list".into()], &s, &mut out), 0);
        assert_eq!(out, "a\ta\n");
    }

    #[test]
    fn list_empty_prints_nothing() {
        let t = TmpDir::new();
        let s = Store::at(t.path());
        let mut out = String::new();
        assert_eq!(run_cli(&["list".into()], &s, &mut out), 0);
        assert!(out.is_empty());
    }

    #[test]
    fn search_cli_finds_line() {
        let t = TmpDir::new();
        let s = Store::at(t.path());
        s.save("a", "needles here\n").unwrap();
        let mut out = String::new();
        assert_eq!(
            run_cli(&["search".into(), "needles".into()], &s, &mut out),
            0
        );
        assert!(out.contains("a:1") && out.contains("needles"));
    }

    #[test]
    fn export_cli_copies() {
        let t = TmpDir::new();
        let s = Store::at(t.path());
        s.save("a", "# note\n").unwrap();
        let dest = t.path().join("out.md");
        let mut out = String::new();
        assert_eq!(
            run_cli(
                &["export".into(), "a".into(), dest.display().to_string()],
                &s,
                &mut out
            ),
            0
        );
        assert_eq!(fs::read_to_string(&dest).unwrap(), "# note\n");
        assert!(out.contains("exported a -> "));
    }

    #[test]
    fn export_missing_pad_is_error() {
        let t = TmpDir::new();
        let s = Store::at(t.path());
        let mut out = String::new();
        assert_eq!(
            run_cli(
                &["export".into(), "nope".into(), "/tmp/x.md".into()],
                &s,
                &mut out
            ),
            1
        );
        assert!(out.contains("export nope"));
    }

    #[test]
    fn unknown_subcommand_is_usage_error() {
        let t = TmpDir::new();
        let mut out = String::new();
        assert_eq!(
            run_cli(&["bogus".into()], &Store::at(t.path()), &mut out),
            2
        );
        assert!(out.contains("usage"));
    }
}
