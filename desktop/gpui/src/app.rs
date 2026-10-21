//! App shell: sidebar navigation + view switching.

use crate::api::Api;
use crate::views::cache::CacheView;
use crate::views::gateway::GatewayView;
use crate::views::instance::{BackToServices, InstanceView};
use crate::views::scopes::ScopesView;
use crate::views::settings::SettingsView;
use crate::views::services::{SelectService, ServicesView};
use crate::views::tools::ToolsView;
#[cfg(debug_assertions)]
use gpui_kit::component::button::Button;
#[cfg(debug_assertions)]
use gpui_kit::component::button::ButtonVariants as _;
use gpui_kit::component::sidebar::{
    Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenuItem, SidebarMenu,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::assets::IconName;
use gpui_kit::{
    div, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render,
    Styled as _, Subscription, Window,
};
use std::sync::Arc;
use gpui_kit::base::{h_flex, v_flex, StyledExt as _};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Services,
    Scopes,
    Tools,
    Gateway,
    Cache,
    Settings,
    Instance,
}

pub struct MainWindow {
    api: Arc<Api>,
    nav: Nav,
    services: Entity<ServicesView>,
    scopes: Entity<ScopesView>,
    tools: Entity<ToolsView>,
    gateway: Entity<GatewayView>,
    cache: Entity<CacheView>,
    settings: Entity<SettingsView>,
    instance: Option<Entity<InstanceView>>,
    instance_sub: Option<Subscription>,
    instance_origin: Nav,
    _open_sub: Subscription,
    _scopes_open_sub: Subscription,
}

impl MainWindow {
    pub fn new(api: Arc<Api>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let services = cx.new(|cx| ServicesView::new(api.clone(), window, cx));
        let scopes = cx.new(|cx| ScopesView::new(api.clone(), window, cx));
        let tools = cx.new(|cx| ToolsView::new(api.clone(), window, cx));
        let gateway = cx.new(|cx| GatewayView::new(api.clone(), window, cx));
        let cache = cx.new(|cx| CacheView::new(api.clone(), window, cx));
        let settings = cx.new(|cx| SettingsView::new(api.clone(), window, cx));

        let open_sub = cx.subscribe_in(
            &services,
            window,
            |this, _view, event: &SelectService, window, cx| {
                this.open_instance(event.0.clone(), Nav::Services, window, cx);
            },
        );
        let scopes_open_sub = cx.subscribe_in(
            &scopes,
            window,
            |this, _view, event: &SelectService, window, cx| {
                this.open_instance(event.0.clone(), Nav::Scopes, window, cx);
            },
        );

        Self {
            api,
            nav: Nav::Services,
            services,
            scopes,
            tools,
            gateway,
            cache,
            settings,
            instance: None,
            instance_sub: None,
            instance_origin: Nav::Services,
            _open_sub: open_sub,
            _scopes_open_sub: scopes_open_sub,
        }
    }

    fn open_instance(
        &mut self,
        addr: crate::api::ServiceAddress,
        origin: Nav,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let instance = cx.new(|cx| InstanceView::new(addr, self.api.clone(), window, cx));
        let back_sub = cx.subscribe_in(
            &instance,
            window,
            |this, _view, _event: &BackToServices, _window, cx| {
                this.nav = this.instance_origin;
                this.instance = None;
                cx.notify();
            },
        );
        self.instance_sub = Some(back_sub);
        self.instance = Some(instance);
        self.instance_origin = origin;
        self.nav = Nav::Instance;
        cx.notify();
    }

    fn nav_item(
        &self,
        label: &'static str,
        icon: IconName,
        nav: Nav,
        cx: &Context<Self>,
    ) -> SidebarMenuItem {
        SidebarMenuItem::new(label)
            .icon(icon)
            .active(self.nav == nav)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.nav = nav;
                cx.notify();
            }))
    }
}

impl Render for MainWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.nav {
            Nav::Services => self.services.clone().into_any_element(),
            Nav::Instance => self
                .instance
                .clone()
                .map(Entity::into_any_element)
                .unwrap_or_else(|| self.services.clone().into_any_element()),
            Nav::Scopes => self.scopes.clone().into_any_element(),
            Nav::Tools => self.tools.clone().into_any_element(),
            Nav::Gateway => self.gateway.clone().into_any_element(),
            Nav::Cache => self.cache.clone().into_any_element(),
            Nav::Settings => self.settings.clone().into_any_element(),
        };

        h_flex()
            .items_stretch()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                Sidebar::new("app-sidebar")
                    .header(SidebarHeader::new().child(
                        h_flex()
                            .gap_2()
                            .child(div().font_semibold().child("mcpstore"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(crate::dict::t(cx, crate::dict::Key::ControlPanel)),
                            ),
                    ))
                    .child(
                        SidebarGroup::new("Main").child(
                            SidebarMenu::new()
                                .child(self.nav_item(
                                    crate::dict::t(cx, crate::dict::Key::Services),
                                    IconName::Server,
                                    Nav::Services,
                                    cx,
                                ))
                                .child(self.nav_item(
                                    crate::dict::t(cx, crate::dict::Key::Scopes),
                                    IconName::Layers,
                                    Nav::Scopes,
                                    cx,
                                ))
                                .child(
                                    self.nav_item(crate::dict::t(cx, crate::dict::Key::Tools), IconName::Wrench, Nav::Tools, cx),
                                )
                                .child(self.nav_item(
                                    "Gateway",
                                    IconName::Network,
                                    Nav::Gateway,
                                    cx,
                                ))
                                .child(self.nav_item(
                                    crate::dict::t(cx, crate::dict::Key::Cache),
                                    IconName::Database,
                                    Nav::Cache,
                                    cx,
                                ))
                                .child(self.nav_item(
                                    crate::dict::t(cx, crate::dict::Key::Settings),
                                    IconName::Settings,
                                    Nav::Settings,
                                    cx,
                                )),
                        ),
                    )
                    .footer(
                        SidebarFooter::new().child({
                            // Debug builds only: click toggles GPUI Inspector (no shortcut needed).
                            // Release strips this button and `toggle_inspector` entirely.
                            let el = v_flex().gap_1().child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(self.api.core_base().to_string()),
                            );
                            #[cfg(debug_assertions)]
                            let el = el.child(
                                Button::new("toggle-inspector")
                                    .ghost()
                                    .icon(IconName::Inspector)
                                    .label("Inspector")
                                    .on_click(|_, window, cx| {
                                        window.toggle_inspector(cx);
                                    }),
                            );
                            el
                        }),
                    ),
            )
            .child(div().flex_1().min_w_0().child(content))
    }
}
