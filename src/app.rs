use adw::prelude::*;

pub const APP_ID: &str = "dev.scratchpad.App";

pub fn run() -> i32 {
    let application = adw::Application::builder().application_id(APP_ID).build();
    application.connect_activate(|app| match app.active_window() {
        Some(w) if w.is_visible() => w.close(),
        _ => crate::ui::build_window(app),
    });
    application.run().into()
}
