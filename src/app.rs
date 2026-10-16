use adw::prelude::*;

pub const APP_ID: &str = "dev.scratchpad.App";

pub fn run() -> i32 {
    let application = adw::Application::builder().application_id(APP_ID).build();
    application.connect_activate(crate::ui::build_window);
    application.run().into()
}
