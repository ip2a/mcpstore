use axum::{routing::get, Router};
use mcpstore::MCPStore;
use std::sync::Arc;

mod actions;
mod assets;
mod components;
mod layout;
mod pages;
mod utils;

pub fn router(store: Arc<MCPStore>) -> Router {
    Router::new()
        .route("/", get(pages::page_home))
        .route("/service/:instance_id", get(pages::page_service))
        .route("/add", get(pages::page_add))
        .route("/add/exec", get(actions::action_add_exec))
        .route("/action/connect/:instance_id", get(actions::action_connect))
        .route(
            "/action/disconnect/:instance_id",
            get(actions::action_disconnect),
        )
        .route("/action/restart/:instance_id", get(actions::action_restart))
        .route("/action/remove/:instance_id", get(actions::action_remove))
        .route(
            "/action/switch-cache-storage",
            get(actions::action_switch_cache_storage),
        )
        .route(
            "/action/switch-backend",
            get(actions::action_switch_cache_storage),
        )
        .route(
            "/modal/switch-cache-storage",
            get(actions::modal_switch_cache_storage),
        )
        .route(
            "/modal/switch-backend",
            get(actions::modal_switch_cache_storage),
        )
        .route(
            "/modal/call-tool/:instance_id/:tool",
            get(actions::modal_call_tool_form),
        )
        .route(
            "/modal/call-tool/:instance_id/:tool/exec",
            get(actions::modal_call_tool_exec),
        )
        .route(
            "/modal/tool-detail/:instance_id/:tool",
            get(actions::modal_tool_detail),
        )
        .route("/assets/style.css", get(assets::serve_css))
        .route("/favicon.ico", get(assets::serve_favicon))
        .with_state(store)
}
