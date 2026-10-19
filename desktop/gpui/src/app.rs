//! App shell: sidebar navigation + view switching.

use crate::api::Api;
use crate::views::cache::CacheView;
use crate::views::instance::{BackToServices, InstanceView};
use crate::views::scopes::ScopesView;
use crate::views::services::{SelectService, ServicesView};
use crate::views::tools::ToolsView;
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
use gpui_kit::base::{h_flex, StyledExt as _};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Services,
    Scopes,
    Tools,
    Cache,
    Instance,
}

pub struct MainWindow {
    api: Arc<Api>,
    nav: Nav,
    services: Entity<ServicesView>,
    scopes: Entity<ScopesView>,
    tools: Entity<ToolsView>,
    cache: Entity<CacheView>,
    instance: Option<Entity<InstanceView>>,
    instance_sub: Option<Subscription>,
    _open_sub: Subscription,
}

impl MainWindow {
    pub fn new(api: Arc<Api>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let services = cx.new(|cx| ServicesView::new(api.clone(), window, cx));
        let scopes = cx.new(|cx| ScopesView::new(api.clone(), window, cx));
        let tools = cx.new(|cx| ToolsView::new(api.clone(), window, cx));
        let cache = cx.new(|cx| CacheView::new(api.clone(), window, cx));

        let open_sub = cx.subscribe_in(
            &services,
            window,
            |this, _view, event: &SelectService, window, cx| {
                let instance = cx.new(|cx| {
                    InstanceView::new(event.0.clone(), this.api.clone(), window, cx)
                });
                let back_sub = cx.subscribe_in(
                    &instance,
                    window,
                    |this, _view, _event: &BackToServices, _window, cx| {
                        this.nav = Nav::Services;
                        this.instance = None;
                        cx.notify();
                    },
                );
                this.instance_sub = Some(back_sub);
                this.instance = Some(instance);
                this.nav = Nav::Instance;
                cx.notify();
            },
        );

        Self {
            api,
            nav: Nav::Services,
            services,
            scopes,
            tools,
            cache,
            instance: None,
            instance_sub: None,
            _open_sub: open_sub,
        }
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
            Nav::Cache => self.cache.clone().into_any_element(),
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
                                    .child("control panel"),
                            ),
                    ))
                    .child(
                        SidebarGroup::new("Main").child(
                            SidebarMenu::new()
                                .child(self.nav_item(
                                    "Services",
                                    IconName::Server,
                                    Nav::Services,
                                    cx,
                                ))
                                .child(self.nav_item(
                                    "Scopes",
                                    IconName::Layers,
                                    Nav::Scopes,
                                    cx,
                                ))
                                .child(
                                    self.nav_item("Tools", IconName::Wrench, Nav::Tools, cx),
                                )
                                .child(self.nav_item(
                                    "Cache",
                                    IconName::Database,
                                    Nav::Cache,
                                    cx,
                                )),
                        ),
                    )
                    .footer(
                        SidebarFooter::new().child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .truncate()
                                .child(self.api.core_base().to_string()),
                        ),
                    ),
            )
            .child(div().flex_1().min_w_0().child(content))
    }
}
