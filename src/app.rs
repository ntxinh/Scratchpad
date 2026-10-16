use adw::prelude::*;

pub const APP_ID: &str = "dev.scratchpad.App";

pub fn run() -> i32 {
    let application = adw::Application::builder().application_id(APP_ID).build();
    application.connect_activate(|app| match app.active_window() {
        Some(w) if w.is_visible() => crate::ui::hide_and_save(&w),
        Some(w) => crate::ui::show_for_cwd(&w),
        None => crate::ui::build_window(app),
    });
    application.run().into()
}
