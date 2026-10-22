//! Home page: overview dashboard — time-range tabs, a call-trend bar chart and
//! four stat cards. All figures are mock data for now; swap `mock::dataset`
//! and the card values for real APIs when the backend exposes them.

use std::sync::Arc;

use crate::api::Api;
use crate::dict::{t, t_args, TextKey};
use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex, StyledExt as _};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::chart::BarChart;
use gpui_kit::component::progress::ProgressCircle;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme, Icon, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    div, px, Context, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Window,
};

/// One bucket of call volume on the trend chart.
struct Point {
    label: &'static str,
    calls: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TimeRange {
    Min10,
    Hour1,
    Day1,
    Week1,
    Month1,
    Custom,
}

impl TimeRange {
    const ALL: [TimeRange; 6] = [
        TimeRange::Min10,
        TimeRange::Hour1,
        TimeRange::Day1,
        TimeRange::Week1,
        TimeRange::Month1,
        TimeRange::Custom,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|r| *r == self).unwrap_or(4)
    }

    fn from_index(ix: usize) -> Option<Self> {
        Self::ALL.get(ix).copied()
    }

    fn key(self) -> TextKey {
        match self {
            TimeRange::Min10 => TextKey::HomeRange10m,
            TimeRange::Hour1 => TextKey::HomeRange1h,
            TimeRange::Day1 => TextKey::HomeRange1d,
            TimeRange::Week1 => TextKey::HomeRange1w,
            TimeRange::Month1 => TextKey::HomeRange1mo,
            TimeRange::Custom => TextKey::HomeRangeCustom,
        }
    }
}

// TODO(api): replace with real metrics once the backend exposes a stats API.
mod mock {
    use super::Point;

    pub const HIT_RATE: f32 = 52.41;
    pub const CALLS_TOTAL: &str = "2.1K";
    pub const CALLS_OK: &str = "1.9K";
    pub const CALLS_ERR: &str = "145";
    pub const TOKENS_TOTAL: &str = "282M";
    pub const TOKENS_PROMPT: &str = "280M";
    pub const VALUE_TOTAL: &str = "$790.59";
    pub const VALUE_CACHE_RW: &str = "$73.53";

    pub const HUB_NAME: &str = "mcpstore";
    pub const HUB_ADDR: &str = "127.0.0.1:1820";
    pub const ROUTE_REQUESTS: &str = "3,391";
    pub const ROUTE_REROUTES: &str = "0";
    pub const ROUTE_ERRORS: &str = "173";
    pub const ROUTE_HIT: &str = "99%";

    /// One routed call: caller agent -> mcpstore -> service.tool.
    pub struct Route {
        pub caller: &'static str,
        pub caller_detail: &'static str,
        pub service: &'static str,
        pub tool: &'static str,
        pub code: u16,
        pub ok: bool,
        pub latency_ms: u64,
    }

    pub fn routes() -> Vec<Route> {
        vec![
            Route {
                caller: "Claude Code",
                caller_detail: "claude-haiku-5-5",
                service: "github",
                tool: "create_issue",
                code: 200,
                ok: true,
                latency_ms: 84,
            },
            Route {
                caller: "Cursor",
                caller_detail: "composer-2.5",
                service: "filesystem",
                tool: "read_file",
                code: 200,
                ok: true,
                latency_ms: 12,
            },
            Route {
                caller: "Claude Code",
                caller_detail: "claude-haiku-5-5",
                service: "database",
                tool: "query",
                code: 503,
                ok: false,
                latency_ms: 1204,
            },
        ]
    }

    /// Daily call volume over the last month, shaped like the reference design.
    fn month() -> Vec<Point> {
        from_raw(&[
            ("9/23", 0), ("9/24", 64), ("9/25", 0), ("9/26", 0), ("9/27", 0),
            ("9/28", 0), ("9/29", 0), ("9/30", 0), ("10/1", 0), ("10/2", 0),
            ("10/3", 0), ("10/4", 0), ("10/5", 0), ("10/6", 0), ("10/7", 0),
            ("10/8", 112), ("10/9", 480), ("10/10", 1536), ("10/11", 640),
            ("10/12", 416), ("10/13", 96), ("10/14", 0), ("10/15", 144),
            ("10/16", 0), ("10/17", 0), ("10/18", 0), ("10/19", 0),
            ("10/20", 0), ("10/21", 176), ("10/22", 192),
        ])
    }

    fn from_raw(raw: &[(&'static str, u64)]) -> Vec<Point> {
        raw.iter()
            .map(|(label, calls)| Point {
                label,
                calls: *calls,
            })
            .collect()
    }

    pub fn dataset(range: super::TimeRange) -> Vec<Point> {
        use super::TimeRange as R;
        match range {
            R::Min10 => from_raw(&[
                ("14:51", 2), ("14:52", 0), ("14:53", 5), ("14:54", 0),
                ("14:55", 8), ("14:56", 0), ("14:57", 3), ("14:58", 12),
                ("14:59", 4), ("15:00", 6),
            ]),
            R::Hour1 => from_raw(&[
                ("14:05", 4), ("14:10", 0), ("14:15", 9), ("14:20", 2),
                ("14:25", 0), ("14:30", 14), ("14:35", 6), ("14:40", 0),
                ("14:45", 11), ("14:50", 7), ("14:55", 3), ("15:00", 8),
            ]),
            R::Day1 => from_raw(&[
                ("0:00", 0), ("1:00", 0), ("2:00", 0), ("3:00", 0), ("4:00", 0),
                ("5:00", 2), ("6:00", 5), ("7:00", 12), ("8:00", 28), ("9:00", 41),
                ("10:00", 36), ("11:00", 30), ("12:00", 22), ("13:00", 27),
                ("14:00", 35), ("15:00", 44), ("16:00", 39), ("17:00", 25),
                ("18:00", 18), ("19:00", 21), ("20:00", 33), ("21:00", 26),
                ("22:00", 9), ("23:00", 3),
            ]),
            R::Week1 => from_raw(&[
                ("周四", 186), ("周五", 240), ("周六", 431), ("周日", 208),
                ("周一", 312), ("周二", 275), ("周三", 368),
            ]),
            R::Month1 | R::Custom => month(),
        }
    }
}

pub struct HomeView {
    // Kept so the real stats API can be wired in without touching app.rs.
    _api: Arc<Api>,
    range: TimeRange,
}

impl HomeView {
    pub fn new(api: Arc<Api>, _window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            _api: api,
            range: TimeRange::Month1,
        }
    }
}

impl Render for HomeView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let success = cx.theme().success;
        let empty_bar = cx.theme().muted;
        let calls_unit = SharedString::from(t(cx, TextKey::HomeCallsUnit));

        let points = mock::dataset(self.range);
        let chart = BarChart::new(points)
            .band(|p: &Point| p.label)
            .value(|p: &Point| p.calls as f64)
            .fill(move |p: &Point, _, _, _| if p.calls > 0 { success } else { empty_bar })
            .tooltip_title(|p: &Point| SharedString::from(p.label))
            .tooltip_value(move |_, v| SharedString::from(format!("{} {}", v as u64, calls_unit)))
            .grid(false)
            .value_axis(false);

        let range_tabs = TabBar::new("home-range-tabs")
            .pill()
            .selected_index(self.range.index())
            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                if let Some(range) = TimeRange::from_index(*ix) {
                    this.range = range;
                    cx.notify();
                }
            }))
            .children(TimeRange::ALL.iter().map(|r| Tab::new().label(t(cx, r.key()))));

        let refresh = Button::new("home-refresh")
            .ghost()
            .icon(IconName::RefreshCw)
            .tooltip(t(cx, TextKey::Refresh))
            // TODO(api): re-fetch stats once real data lands.
            .on_click(cx.listener(|_, _, _, cx| cx.notify()));

        // Stat card: label + info tooltip, big value, muted footnote.
        let card = |id: &'static str,
                    label: &'static str,
                    hint: &'static str,
                    body: gpui_kit::AnyElement,
                    footnote: SharedString| {
            v_flex()
                .id(id)
                .flex_1()
                .min_w(px(190.))
                .gap_2()
                .px_4()
                .py_3()
                .border_1()
                .border_color(border)
                .rounded_md()
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(div().text_sm().text_color(muted).child(label))
                        .child(
                            Button::new(id)
                                .ghost()
                                .xsmall()
                                .icon(IconName::Info)
                                .tooltip(hint),
                        ),
                )
                .child(body)
                .when(!footnote.is_empty(), |el| {
                    el.child(div().text_xs().text_color(muted).child(footnote))
                })
        };

        let gauge = ProgressCircle::new("home-hit-gauge")
            .value(mock::HIT_RATE)
            .color(success)
            .w(px(76.))
            .h(px(76.))
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .child(format!("{:.2}%", mock::HIT_RATE)),
            );

        let value_body = |text: &'static str| div().text_2xl().font_semibold().child(text);

        let cards = h_flex()
            .gap_3()
            .flex_wrap()
            .child(card(
                "card-hit",
                t(cx, TextKey::HomeCacheHitRate),
                t(cx, TextKey::HintCacheHitRate),
                gauge.into_any_element(),
                SharedString::default(),
            ))
            .child(card(
                "card-calls",
                t(cx, TextKey::HomeLlmCalls),
                t(cx, TextKey::HintLlmCalls),
                value_body(mock::CALLS_TOTAL).into_any_element(),
                SharedString::from(t_args(
                    cx,
                    TextKey::HomeCallsDetail,
                    &[("ok", mock::CALLS_OK), ("err", mock::CALLS_ERR)],
                )),
            ))
            .child(card(
                "card-tokens",
                t(cx, TextKey::HomeTokens),
                t(cx, TextKey::HintTokens),
                value_body(mock::TOKENS_TOTAL).into_any_element(),
                SharedString::from(t_args(
                    cx,
                    TextKey::HomePromptTokens,
                    &[("count", mock::TOKENS_PROMPT)],
                )),
            ))
            .child(card(
                "card-value",
                t(cx, TextKey::HomeValueEstimate),
                t(cx, TextKey::HintValueEstimate),
                value_body(mock::VALUE_TOTAL).into_any_element(),
                SharedString::from(t_args(
                    cx,
                    TextKey::HomeCacheRw,
                    &[("amount", mock::VALUE_CACHE_RW)],
                )),
            ));

        // Call-routing card: caller agent -> mcpstore -> service.tool.
        let routes = mock::routes();
        let route_node = |icon: IconName, name: &'static str, detail: &'static str, accent: Hsla| {
            h_flex()
                .gap_2()
                .items_center()
                .min_w(px(150.))
                .px_3()
                .py_2()
                .border_1()
                .border_color(accent)
                .rounded_md()
                .child(Icon::new(icon).text_color(muted))
                .child(
                    v_flex()
                        .child(div().text_sm().font_medium().child(name))
                        .child(div().text_xs().text_color(muted).child(detail)),
                )
        };
        let route_rows = v_flex().gap_2().children(routes.iter().map(|r| {
            let status = if r.ok {
                t(cx, TextKey::HomeRouteOk)
            } else {
                t(cx, TextKey::HomeRouteFail)
            };
            let status_text = SharedString::from(format!(
                "{} · {} · {} · {}ms",
                r.code,
                status,
                t_args(cx, TextKey::HomeRouteReturned, &[("caller", r.caller)]),
                r.latency_ms
            ));
            let status_color = if r.ok { success } else { cx.theme().danger };
            h_flex()
                .items_center()
                .child(route_node(IconName::Bot, r.caller, r.caller_detail, border))
                .child(div().w(px(24.)).h(px(1.)).bg(border))
                .child(route_node(
                    IconName::Network,
                    mock::HUB_NAME,
                    mock::HUB_ADDR,
                    cx.theme().primary,
                ))
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .border_1()
                        .border_dashed()
                        .border_color(border)
                        .rounded_md()
                        .child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .min_w_0()
                                .child(Icon::new(IconName::Hammer).text_color(muted))
                                .child(
                                    div()
                                        .text_sm()
                                        .truncate()
                                        .child(format!("{}@{}", r.service, r.tool)),
                                ),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(status_color)
                                .child(status_text),
                        ),
                )
        }));
        let latest = &routes[0];
        let route_target = format!("{}@{}", latest.service, latest.tool);
        let route_ms = latest.latency_ms.to_string();
        let route_summary = t_args(
            cx,
            TextKey::HomeRouteSummary,
            &[
                ("caller", latest.caller),
                ("target", route_target.as_str()),
                ("ms", route_ms.as_str()),
                ("hit", mock::ROUTE_HIT),
            ],
        );
        let route_stat = |num: &'static str, label: &'static str| {
            v_flex()
                .items_end()
                .child(div().text_lg().font_semibold().child(num))
                .child(div().text_xs().text_color(muted).child(label))
        };
        let route_card = v_flex()
            .w_full()
            .border_1()
            .border_color(border)
            .rounded_md()
            .p_3()
            .gap_3()
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_medium()
                            .child(t(cx, TextKey::HomeRouteTitle)),
                    )
                    .child(
                        Button::new("hint-route")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Info)
                            .tooltip(t(cx, TextKey::HintRoute)),
                    ),
            )
            .child(route_rows)
            .child(div().h(px(1.)).w_full().bg(border))
            .child(
                h_flex()
                    .items_end()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_sm()
                            .text_color(muted)
                            .child(route_summary),
                    )
                    .child(
                        h_flex()
                            .gap_6()
                            .child(route_stat(
                                mock::ROUTE_REQUESTS,
                                t(cx, TextKey::HomeRouteRequests),
                            ))
                            .child(route_stat(
                                mock::ROUTE_REROUTES,
                                t(cx, TextKey::HomeRouteReroutes),
                            ))
                            .child(route_stat(
                                mock::ROUTE_ERRORS,
                                t(cx, TextKey::HomeRouteErrors),
                            )),
                    ),
            );

        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(border)
                    .child(div().font_semibold().child(t(cx, TextKey::Overview)))
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child(t(cx, TextKey::HomeMockHint)),
                    )
                    .child(div().flex_1())
                    .child(range_tabs)
                    .child(refresh),
            )
            .child(
                div()
                    .id("home-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .p_3()
                            .gap_4()
                            .child(
                                div()
                                    .w_full()
                                    .border_1()
                                    .border_color(border)
                                    .rounded_md()
                                    .p_3()
                                    .child(div().w_full().h(px(230.)).child(chart)),
                            )
                            .child(cards)
                            .child(route_card),
                    ),
            )
    }
}
