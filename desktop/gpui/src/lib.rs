//! mcpstore GPUI control panel — shared app bootstrap for desktop and web (wasm).

pub mod api;
pub mod app;
pub mod dict;
pub mod dialogs;
pub mod views;

use gpui_kit::{App, AppContext as _, WindowOptions};
use std::sync::Arc;

/// Build the API client and open the main window. Runs inside `Application::run`.
pub fn launch_panel(cx: &mut App) {
    gpui_kit::init(cx);
    dict::init(dict::Lang::Auto, cx);
    let api = Arc::new(api::Api::new("http://127.0.0.1:1820", "http://127.0.0.1:1821"));
    gpui_kit::open_window(WindowOptions::default(), cx, move |window, cx| {
        cx.new(|cx| app::MainWindow::new(api.clone(), window, cx))
    })
    .expect("failed to open panel window");
    cx.activate(true);
}

#[cfg(target_family = "wasm")]
mod wasm_entry {
    use super::*;
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;

    thread_local! {
        static APPLICATION: RefCell<Option<gpui_kit::ApplicationHandle>> = const { RefCell::new(None) };
    }

    #[wasm_bindgen]
    pub fn start() -> Result<(), JsValue> {
        gpui_kit::platform::web_init();
        // The web platform ships no fonts; .SystemUIFont resolves to IBM Plex Sans
        // and the first text layout panics without it. NotoSansSC covers zh labels.
        const FONTS: &[&[u8]] = &[
            include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf").as_slice(),
            include_bytes!("../assets/fonts/NotoSansSC-Regular.ttf").as_slice(),
        ];
        // wasm Assets 从 {endpoint}/assets/icons/*.svg 拉图标；endpoint 用页面同源。
        let origin = web_sys::window()
            .map(|w| w.location().origin().unwrap_or_default())
            .unwrap_or_default();
        let application = gpui_kit::platform::single_threaded_web()
            .with_assets(gpui_kit::assets::Assets::new(origin))
            .run_embedded(move |cx: &mut App| {
            cx.text_system()
                .add_fonts(FONTS.iter().map(|f| std::borrow::Cow::Borrowed(*f)).collect())
                .expect("failed to load bundled fonts");
            launch_panel(cx);
        });
        APPLICATION.with(|slot| *slot.borrow_mut() = Some(application));
        Ok(())
    }
}
