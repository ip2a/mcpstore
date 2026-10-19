//! mcpstore GPUI desktop control panel.

mod api;
mod app;
mod views;

use gpui_kit::{AppContext as _, WindowOptions};
use std::sync::Arc;

fn main() {
    let api = Arc::new(api::Api::new("http://127.0.0.1:1820"));
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            let api = api.clone();
            gpui_kit::open_window(WindowOptions::default(), cx, move |window, cx| {
                cx.new(|cx| app::MainWindow::new(api.clone(), window, cx))
            })
            .expect("failed to open window");
        });
}
