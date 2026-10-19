//! Desktop entry point. The wasm entry lives in the library (`lib.rs`).

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(mcpstore_desktop::launch_panel);
}
