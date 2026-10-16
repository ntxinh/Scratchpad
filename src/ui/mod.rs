use std::cell::RefCell;

use adw::prelude::*;
use gtk4_layer_shell::{Edge, Layer, LayerShell};

thread_local! {
    static WINDOW: RefCell<Option<adw::ApplicationWindow>> = const { RefCell::new(None) };
}

pub fn build_window(app: &adw::Application) {
    WINDOW.with(|cell| {
        if let Some(window) = cell.borrow().as_ref() {
            window.present();
            return;
        }

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
        *cell.borrow_mut() = Some(window);
    });
}
