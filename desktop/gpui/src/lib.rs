//! mcpstore GPUI control panel — shared app bootstrap for desktop and web (wasm).

pub mod api;
pub mod app;
pub mod dict;
pub mod dialogs;
pub mod service_form;
pub mod views;

use gpui_kit::{App, AppContext as _, WindowOptions};
use std::sync::Arc;

/// Build the API client and open the main window. Runs inside `Application::run`.
pub fn launch_panel(cx: &mut App) {
    gpui_kit::init(cx);
    // Temporary: hide outward focus rings to evaluate a quieter control look.
    // Focused inputs still tint their border via theme.ring.
    gpui_kit::component::Theme::update(cx, |theme| {
        theme.focus_ring = false;
    });
    dict::init(dict::Lang::Auto, cx);
    let api = Arc::new(api::Api::new("http://127.0.0.1:1820", "http://127.0.0.1:1821"));
    gpui_kit::open_window(panel_window_options(cx), cx, move |window, cx| {
        cx.new(|cx| app::MainWindow::new(api.clone(), window, cx))
    })
    .expect("failed to open panel window");
    cx.activate(true);
}

/// Same default size as the React shell in `desktop/tauri/src/lib.rs`.
/// Placement follows the cross-platform window-state record: normal x/y/size,
/// plus maximized/fullscreen, on a stable display id. GPUI's `Window::window_bounds`
/// is that record (Win32 `WINDOWPLACEMENT`: the normal rect, not the zoomed one).
/// A rect that no longer overlaps a connected display is centered instead.
fn panel_window_options(cx: &App) -> WindowOptions {
    #[cfg(target_family = "wasm")]
    {
        let _ = cx;
        WindowOptions::default()
    }
    #[cfg(not(target_family = "wasm"))]
    {
        use gpui_kit::{px, size, Bounds, DisplayId, WindowBounds};
        let work_area = cx
            .primary_display()
            .map(|display| display.visible_bounds().size.width.as_f32());
        let min_w = min_inner_width(work_area);
        const MIN_H: f32 = 480.;
        let saved = saved_panel_window();
        let display = saved.as_ref().and_then(|state| {
            state.display.as_deref().and_then(|uuid| {
                cx.displays().into_iter().find(|display| {
                    display
                        .uuid()
                        .ok()
                        .is_some_and(|id| id.to_string() == uuid)
                })
            })
        });
        let display = display.or_else(|| cx.primary_display());
        let (box_w, box_h) = display
            .as_ref()
            .map(|display| {
                let size = display.bounds().size;
                (size.width.as_f32(), size.height.as_f32())
            })
            .unwrap_or((1180., 760.));
        let mut options = WindowOptions::default();
        if let Some(display) = display.as_ref() {
            options.display_id = Some(display.id());
        }
        options.window_bounds = Some(match saved.and_then(|state| {
            restore_panel_window(&state, box_w, box_h, min_w, MIN_H)
        }) {
            Some(restored) => {
                let bounds = Bounds::new(
                    gpui_kit::point(px(restored.x), px(restored.y)),
                    size(px(restored.width), px(restored.height)),
                );
                if restored.fullscreen {
                    WindowBounds::Fullscreen(bounds)
                } else if restored.maximized {
                    WindowBounds::Maximized(bounds)
                } else {
                    WindowBounds::Windowed(bounds)
                }
            }
            None => {
                let id: Option<DisplayId> = display.as_ref().map(|display| display.id());
                WindowBounds::Windowed(Bounds::centered(
                    id,
                    size(px(1180.), px(760.)),
                    cx,
                ))
            }
        });
        options.window_min_size = Some(size(px(min_w), px(MIN_H)));
        options
    }
}

/// Min width: 70% of the 1280px content column + 24px padding, floor 720,
/// and never wider than 94% of the work area.
fn min_inner_width(work_area_width: Option<f32>) -> f32 {
    let mut min_w = (1280.0f32 * 0.70 + 24.0).max(720.0);
    if let Some(area) = work_area_width.filter(|width| *width > 0.0) {
        min_w = min_w.min(area * 0.94);
    }
    min_w
}

/// At least this much of the saved window must still lie on a display, or we center it.
const MIN_ON_SCREEN: f32 = 48.0;

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct PanelWindowState {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    maximized: bool,
    fullscreen: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    display: Option<String>,
}

struct RestoredWindow {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    maximized: bool,
    fullscreen: bool,
}

fn state_sane(state: &PanelWindowState) -> bool {
    [state.x, state.y, state.width, state.height]
        .iter()
        .all(|value| value.is_finite())
        && (-20_000.0..=20_000.0).contains(&state.x)
        && (-20_000.0..=20_000.0).contains(&state.y)
        && (100.0..=10_000.0).contains(&state.width)
        && (100.0..=10_000.0).contains(&state.height)
}

/// `None` when the saved rect misses every display: caller centers the default size.
fn restore_panel_window(
    state: &PanelWindowState,
    display_w: f32,
    display_h: f32,
    min_w: f32,
    min_h: f32,
) -> Option<RestoredWindow> {
    if !state_sane(state) || display_w <= 0.0 || display_h <= 0.0 {
        return None;
    }
    let overlap_w = (state.x + state.width).min(display_w) - state.x.max(0.0);
    let overlap_h = (state.y + state.height).min(display_h) - state.y.max(0.0);
    if overlap_w < MIN_ON_SCREEN || overlap_h < MIN_ON_SCREEN {
        return None;
    }
    Some(RestoredWindow {
        x: state.x,
        y: state.y,
        width: state.width.clamp(min_w, display_w.max(min_w)),
        height: state.height.clamp(min_h, display_h.max(min_h)),
        maximized: state.maximized && !state.fullscreen,
        fullscreen: state.fullscreen,
    })
}

#[cfg(not(target_family = "wasm"))]
fn panel_window_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(
        std::path::PathBuf::from(home)
            .join(".mcpstore")
            .join("panel-window.json"),
    )
}

#[cfg(not(target_family = "wasm"))]
fn saved_panel_window() -> Option<PanelWindowState> {
    let state: PanelWindowState = serde_json::from_str(&std::fs::read_to_string(panel_window_path()?).ok()?).ok()?;
    state_sane(&state).then_some(state)
}

#[cfg(not(target_family = "wasm"))]
fn write_panel_window(state: &PanelWindowState) {
    let Some(path) = panel_window_path() else {
        return;
    };
    let Ok(text) = serde_json::to_string(state) else {
        return;
    };
    if std::fs::write(&path, &text).is_err() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, text);
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn remember_panel_window(window: &gpui_kit::Window, cx: &App) {
    use gpui_kit::WindowBounds;
    static LAST: std::sync::Mutex<Option<PanelWindowState>> = std::sync::Mutex::new(None);
    let mut last = LAST.lock().unwrap_or_else(|poison| poison.into_inner());
    let mut next = last.clone().unwrap_or_else(|| {
        saved_panel_window().unwrap_or(PanelWindowState {
            x: 0.0,
            y: 0.0,
            width: 1180.0,
            height: 760.0,
            maximized: false,
            fullscreen: false,
            display: None,
        })
    });
    match window.window_bounds() {
        WindowBounds::Maximized(bounds) | WindowBounds::Fullscreen(bounds) => {
            next.x = bounds.origin.x.as_f32().round();
            next.y = bounds.origin.y.as_f32().round();
            next.width = bounds.size.width.as_f32().round();
            next.height = bounds.size.height.as_f32().round();
            next.fullscreen = matches!(window.window_bounds(), WindowBounds::Fullscreen(_));
            next.maximized = !next.fullscreen;
        }
        WindowBounds::Windowed(bounds) => {
            if window.is_maximized() {
                // macOS zoom reports the filled frame, not the restore rect.
                next.maximized = true;
                next.fullscreen = false;
            } else if window.is_fullscreen() {
                next.fullscreen = true;
                next.maximized = false;
            } else {
                let size = window.viewport_size();
                next.x = bounds.origin.x.as_f32().round();
                next.y = bounds.origin.y.as_f32().round();
                next.width = size.width.as_f32().round();
                next.height = size.height.as_f32().round();
                next.maximized = false;
                next.fullscreen = false;
            }
        }
    }
    next.display = window
        .display(cx)
        .and_then(|display| display.uuid().ok().map(|id| id.to_string()));
    if !state_sane(&next) {
        return;
    }
    if last.as_ref() == Some(&next) {
        return;
    }
    *last = Some(next.clone());
    write_panel_window(&next);
}

#[cfg(test)]
mod tests {
    use super::{min_inner_width, restore_panel_window, PanelWindowState};

    #[test]
    fn min_width_matches_tauri_shell() {
        let boundary = 1280.0f32 * 0.70 + 24.0;
        assert_eq!(min_inner_width(None), boundary);
        assert_eq!(min_inner_width(Some(10_000.0)), boundary);
        assert_eq!(min_inner_width(Some(800.0)), 800.0 * 0.94);
        assert_eq!(min_inner_width(Some(0.0)), boundary);
    }

    fn sample(x: f32, y: f32) -> PanelWindowState {
        PanelWindowState {
            x,
            y,
            width: 1180.0,
            height: 760.0,
            maximized: false,
            fullscreen: false,
            display: None,
        }
    }

    #[test]
    fn restore_keeps_a_window_that_still_overlaps_the_display() {
        let restored = restore_panel_window(&sample(40.0, 20.0), 1440.0, 900.0, 720.0, 480.0)
            .expect("on screen");
        assert_eq!((restored.x, restored.y, restored.width, restored.height), (40.0, 20.0, 1180.0, 760.0));
    }

    #[test]
    fn restore_centers_when_the_display_is_gone() {
        assert!(restore_panel_window(&sample(-4000.0, 10.0), 1440.0, 900.0, 720.0, 480.0).is_none());
    }

    #[test]
    fn restore_clamps_to_the_display_and_keeps_maximized() {
        let mut state = sample(10.0, 10.0);
        state.width = 4000.0;
        state.maximized = true;
        let restored = restore_panel_window(&state, 1440.0, 900.0, 720.0, 480.0).expect("on screen");
        assert_eq!(restored.width, 1440.0);
        assert!(restored.maximized);
    }
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
