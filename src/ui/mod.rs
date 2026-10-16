mod preview;

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
    static STACK: RefCell<Option<gtk4::Stack>> = const { RefCell::new(None) };
    static PAD_BTN: RefCell<Option<gtk4::Button>> = const { RefCell::new(None) };
    static STORE: Store = Store::new();
    static PAD: RefCell<String> = RefCell::new("default".into());
    static SAVE_SRC: RefCell<Option<glib::SourceId>> = const { RefCell::new(None) };
    // Suppresses the autosave debounce while we load a pad into the buffer —
    // writing back identical text would needlessly bump `modified`.
    static LOADING: Cell<bool> = const { Cell::new(false) };
}

/// Swap `id`'s text into the buffer under the LOADING guard and
/// reflect it in the headerbar's pad button.
fn load_pad(id: &str) {
    PAD.with(|p| *p.borrow_mut() = id.to_string());
    BUFFER.with(|b| {
        if let Some(buf) = b.borrow().as_ref() {
            let text = STORE.with(|s| s.load(id));
            LOADING.with(|l| l.set(true));
            buf.set_text(&text);
            LOADING.with(|l| l.set(false));
        }
    });
    PAD_BTN.with(|b| {
        if let Some(btn) = b.borrow().as_ref() {
            btn.set_label(id);
        }
    });
    // Always reopen on the editor — a stale preview of the previous pad
    // would otherwise greet the user after a pad switch.
    STACK.with(|s| {
        if let Some(stack) = s.borrow().as_ref() {
            stack.set_visible_child_name("edit");
        }
    });
}

/// Refill the switcher list: pads whose id/name contains `term`, then
/// per-line hits from `Store::search`. Every row carries its pad id.
fn refresh_pad_list(list: &gtk4::ListBox, term: &str) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }
    let term_l = term.to_lowercase();
    for meta in STORE.with(|s| s.list()) {
        if !term_l.is_empty()
            && !meta.id.to_lowercase().contains(&term_l)
            && !meta.name.to_lowercase().contains(&term_l)
        {
            continue;
        }
        let label = if meta.name == meta.id {
            meta.id.clone()
        } else {
            format!("{} — {}", meta.id, meta.name)
        };
        let row = gtk4::ListBoxRow::new();
        row.set_child(Some(
            &gtk4::Label::builder().label(label).xalign(0.0).build(),
        ));
        unsafe { row.set_data("pad-id", meta.id) };
        list.append(&row);
    }
    if !term.is_empty() {
        for (id, line, text) in STORE.with(|s| s.search(term)) {
            let row = gtk4::ListBoxRow::new();
            row.set_child(Some(
                &gtk4::Label::builder()
                    .label(format!("{id}:{line}  {}", text.trim()))
                    .xalign(0.0)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .build(),
            ));
            row.add_css_class("dim-label");
            unsafe { row.set_data("pad-id", id) };
            list.append(&row);
        }
    }
}

/// Popover holding the search entry + pad/hit list, parented to `pad_btn`.
fn build_pad_popover(pad_btn: &gtk4::Button) -> gtk4::Popover {
    let search = gtk4::SearchEntry::new();
    let list = gtk4::ListBox::new();
    list.add_css_class("navigation-sidebar");
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(320)
        .child(&list)
        .build();
    let vbox = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(6)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(6)
        .margin_end(6)
        .build();
    vbox.append(&search);
    vbox.append(&scroll);

    let pop = gtk4::Popover::builder()
        .position(gtk4::PositionType::Bottom)
        .child(&vbox)
        .build();
    pop.set_parent(pad_btn);

    // Fresh rebuild on every open — other invocations may have added pads.
    pop.connect_map({
        let list = list.clone();
        let search = search.clone();
        move |_| {
            search.set_text("");
            refresh_pad_list(&list, "");
            search.grab_focus();
        }
    });
    search.connect_search_changed({
        let list = list.clone();
        move |s| refresh_pad_list(&list, &s.text())
    });

    list.connect_row_activated({
        let pop = pop.clone();
        move |_, row| {
            // Rows always carry a pad-id; missing data would be a bug, skip.
            let Some(id) = (unsafe { row.data::<String>("pad-id") }) else {
                return;
            };
            let id: String = unsafe { id.as_ref() }.clone();
            if PAD.with(|p| *p.borrow() == id) {
                pop.popdown();
                return;
            }
            flush_save();
            load_pad(&id);
            pop.popdown();
        }
    });

    pop
}

fn save_now(buf: &gtk4::TextBuffer) {
    let text = buf.text(&buf.start_iter(), &buf.end_iter(), false);
    STORE.with(|s| {
        PAD.with(|p| {
            if let Err(e) = s.save(&p.borrow(), &text) {
                eprintln!("scratchpad: save failed: {e}");
            }
        })
    });
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
    load_pad(&pad);
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
        let preview_view = gtk4::TextView::builder()
            .editable(false)
            .cursor_visible(false)
            .build();
        let stack = gtk4::Stack::new();
        stack.add_named(
            &gtk4::ScrolledWindow::builder().child(&view).build(),
            Some("edit"),
        );
        stack.add_named(
            &gtk4::ScrolledWindow::builder().child(&preview_view).build(),
            Some("preview"),
        );

        // Headerbar: pad switcher on the left, quick actions on the right.
        let header = adw::HeaderBar::new();
        let pad_btn = gtk4::Button::builder().label("default").build();
        let pop = build_pad_popover(&pad_btn);
        pad_btn.connect_clicked(move |_| pop.popup());
        header.pack_start(&pad_btn);

        let fence_btn = gtk4::Button::builder()
            .label("```")
            .tooltip_text("Insert fenced code block")
            .build();
        fence_btn.connect_clicked({
            let view = view.clone();
            move |_| view.buffer().insert_at_cursor("\n```\n\n```\n")
        });
        let todo_btn = gtk4::Button::builder()
            .label("- [ ]")
            .tooltip_text("Insert todo checkbox")
            .build();
        todo_btn.connect_clicked({
            let view = view.clone();
            move |_| {
                let buf = view.buffer();
                let mut iter = buf.iter_at_mark(&buf.get_insert());
                iter.set_line_offset(0);
                buf.insert(&mut iter, "- [ ] ");
            }
        });
        let url_btn = gtk4::Button::builder()
            .label("[]()")
            .tooltip_text("Insert markdown link")
            .build();
        url_btn.connect_clicked({
            let view = view.clone();
            move |_| {
                let buf = view.buffer();
                buf.insert_at_cursor("[]()");
                // Land between the brackets so the title is typed first.
                let mut iter = buf.iter_at_mark(&buf.get_insert());
                iter.backward_chars(3);
                buf.place_cursor(&iter);
            }
        });
        let cmd_btn = gtk4::Button::builder()
            .label("`cmd`")
            .tooltip_text("Insert inline command")
            .build();
        cmd_btn.connect_clicked({
            let view = view.clone();
            move |_| {
                let buf = view.buffer();
                buf.insert_at_cursor("`cmd`");
                // Select "cmd" so typing replaces it.
                let mut start = buf.iter_at_mark(&buf.get_insert());
                start.backward_chars(4);
                buf.select_range(&start, &buf.iter_at_mark(&buf.get_insert()));
            }
        });
        let copy_btn = gtk4::Button::builder()
            .label("Copy")
            .tooltip_text("Copy entire pad")
            .build();
        copy_btn.connect_clicked(|b| {
            BUFFER.with(|c| {
                if let Some(buf) = c.borrow().as_ref() {
                    let text = buf.text(&buf.start_iter(), &buf.end_iter(), false);
                    b.clipboard().set_text(&text);
                }
            });
        });
        header.pack_end(&copy_btn);
        header.pack_end(&todo_btn);
        header.pack_end(&fence_btn);
        header.pack_end(&url_btn);
        header.pack_end(&cmd_btn);

        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&header);
        toolbar.set_content(Some(&stack));
        window.set_content(Some(&toolbar));

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

        // Ctrl+P toggles between editor and rendered markdown preview.
        let keys = gtk4::EventControllerKey::new();
        keys.connect_key_pressed({
            let stack = stack.clone();
            move |_, key, _, state| {
                let is_p = key == gtk4::gdk::Key::p || key == gtk4::gdk::Key::P;
                if !is_p || !state.contains(gtk4::gdk::ModifierType::CONTROL_MASK) {
                    return glib::Propagation::Proceed;
                }
                if stack.visible_child_name().as_deref() == Some("preview") {
                    stack.set_visible_child_name("edit");
                    view.grab_focus();
                } else {
                    let buf = view.buffer();
                    let md = buf.text(&buf.start_iter(), &buf.end_iter(), false);
                    preview::render_markdown(&md, &preview_view.buffer());
                    stack.set_visible_child_name("preview");
                }
                glib::Propagation::Stop
            }
        });
        window.add_controller(keys);

        // Stay resident: close just hides + saves, never destroys.
        window.connect_close_request(|w| {
            hide_and_save(w.upcast_ref());
            glib::Propagation::Stop
        });

        let window: adw::ApplicationWindow = window;
        *cell.borrow_mut() = Some(window.clone());
        BUFFER.with(|b| *b.borrow_mut() = Some(buffer));
        STACK.with(|s| *s.borrow_mut() = Some(stack.clone()));
        PAD_BTN.with(|b| *b.borrow_mut() = Some(pad_btn));
        show_for_cwd(window.upcast_ref());
    });
}
