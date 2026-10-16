use adw::prelude::*;

pub const APP_ID: &str = "dev.scratchpad.App";

pub fn run() -> i32 {
    let application = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gtk4::gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    application.connect_activate(|app| match app.active_window() {
        Some(w) if w.is_visible() => crate::ui::hide_and_save(&w),
        Some(w) => crate::ui::show_for_cwd(&w),
        None => crate::ui::build_window(app),
    });
    // Remote invocations land here: caller's cwd picks the pad.
    application.connect_command_line(|app, cl| {
        match app.active_window() {
            Some(w) if w.is_visible() => crate::ui::hide_and_save(&w),
            Some(w) => {
                let cwd = cl.cwd().unwrap_or_default();
                crate::ui::show_at(&w, &cwd)
            }
            None => crate::ui::build_window(app),
        }
        0
    });
    // Flush pending debounced save on exit (SIGTERM etc.).
    application.connect_shutdown(|_| crate::ui::flush_save());
    application.run().into()
}
