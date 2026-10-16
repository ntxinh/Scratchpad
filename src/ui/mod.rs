use std::cell::{Cell, RefCell};
use std::path::Path;
use std::time::Duration;

use adw::prelude::*;
use gtk4::glib;
use gtk4_layer_shell::{Edge, Layer, LayerShell};

use crate::detect;
use crate::store::Store;

thread_local! {
    static WINDOW: RefCell<Option<adw::ApplicationWindow>> = const { RefCell::new(None) };
    static BUFFER: RefCell<Option<gtk4::TextBuffer>> = const { RefCell::new(None) };
    static STORE: Store = Store::new();
    static PAD: RefCell<String> = RefCell::new("default".into());
    static SAVE_SRC: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
    // Suppresses the autosave debounce while we load a pad into the buffer —
    // writing back identical text would needlessly bump `modified`.
    static LOADING: Cell<bool> = const { Cell::new(false) };
}

fn save_now(buf: &gtk4::TextBuffer) {
    let text = buf.text(&buf.start_iter(), &buf.end_iter(), false);
    STORE.with(|s| PAD.with(|p| s.save(&p.borrow(), &text).ok()));
}

fn cancel_pending_save() {
    SAVE_SRC.with(|s| {
        if let Some(id) = s.borrow_mut().take() {
            id.remove();
        }
    });
}

pub fn flush_save() {
    cancel_pending_save();
    BUFFER.with(|b| {
        if let Some(buf) = b.borrow().as_ref() {
            save_now(buf);
        }
    });
}

pub fn hide_and_save(w: &gtk4::Window) {
    flush_save();
    w.set_visible(false);
}

pub fn show_for_cwd(w: &gtk4::Window) {
    let cwd = std::env::current_dir().unwrap_or_default();
    show_at(w, &cwd);
}

pub fn show_at(w: &gtk4::Window, cwd: &Path) {
    let pad = detect::pad_id(cwd);
    PAD.with(|p| *p.borrow_mut() = pad.clone());
    if let Some(root) = detect::repo_root(cwd) {
        let unset = STORE.with(|s| {
            s.list()
                .iter()
                .find(|m| m.id == pad)
                .and_then(|m| m.repo_path.as_ref())
                .is_none()
        });
        if unset {
            STORE.with(|s| s.set_repo(&pad, &root));
        }
    }
    BUFFER.with(|b| {
        if let Some(buf) = b.borrow().as_ref() {
            let text = STORE.with(|s| s.load(&pad));
            LOADING.with(|l| l.set(true));
            buf.set_text(&text);
            LOADING.with(|l| l.set(false));
        }
    });
    w.present();
}

pub fn build_window(app: &adw::Application) {
    WINDOW.with(|cell| {
        if let Some(window) = cell.borrow().as_ref() {
            show_for_cwd(window.upcast_ref());
            return;
        }

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Scratchpad")
            .default_width(560)
            .default_height(380)
            .build();

        window.connect_destroy(|_| {
            WINDOW.with(|cell| *cell.borrow_mut() = None);
        });

        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_anchor(Edge::Top, true);
        window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::OnDemand);
        window.set_margin(Edge::Top, 80);

        let view = gtk4::TextView::builder().monospace(true).build();
        let scroll = gtk4::ScrolledWindow::builder().child(&view).build();
        window.set_content(Some(&scroll));

        let buffer = view.buffer();
        buffer.connect_changed(|buf| {
            if LOADING.with(Cell::get) {
                return;
            }
            cancel_pending_save();
            let b = buf.clone();
            SAVE_SRC.with(|s| {
                *s.borrow_mut() = Some(glib::timeout_add_local_once(
                    Duration::from_millis(500),
                    move || {
                        // Already dispatched: take() so remove() is never
                        // called on a dead SourceId (it panics).
                        SAVE_SRC.with(|s| s.borrow_mut().take());
                        save_now(&b);
                    },
                ));
            });
        });

        // Stay resident: close just hides + saves, never destroys.
        window.connect_close_request(|w| {
            hide_and_save(w.upcast_ref());
            glib::Propagation::Stop
        });

        let window: adw::ApplicationWindow = window;
        *cell.borrow_mut() = Some(window.clone());
        BUFFER.with(|b| *b.borrow_mut() = Some(buffer));
        show_for_cwd(window.upcast_ref());
    });
}
