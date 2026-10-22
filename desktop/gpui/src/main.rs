//! Desktop entry point. The wasm entry lives in the library (`lib.rs`).

use gpui_kit::AssetSource;

// `Assets` embeds only gpui-kit `default-icons.txt`. These panel glyphs are
// outside that list and otherwise render blank.
gpui_kit::assets::icon_assets!(
    PanelIcons,
    [Activity, Database, GripVertical, Layers, Pencil, Server, Trash, Wrench]
);

struct PanelAssets;

impl AssetSource for PanelAssets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        if let Some(bytes) = PanelIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<gpui_kit::SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(PanelIcons.list(path)?);
        Ok(paths)
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(PanelAssets)
        .run(mcpstore_desktop::launch_panel);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_icons_resolve() {
        let src = PanelAssets;
        for path in [
            "icons/activity.svg",
            "icons/database.svg",
            "icons/grip-vertical.svg",
            "icons/layers.svg",
            "icons/pencil.svg",
            "icons/server.svg",
            "icons/trash.svg",
            "icons/wrench.svg",
            "icons/search.svg",
        ] {
            let bytes = src.load(path).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert!(bytes.as_ref().is_some_and(|b| !b.is_empty()), "{path} missing");
        }
    }
}
