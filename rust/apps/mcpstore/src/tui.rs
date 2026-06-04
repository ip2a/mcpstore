use std::io::{self, Stdout};
use std::time::Duration;

use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use mcpstore::registry::{ConnectionStatus, ServiceEntry};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState, Wrap},
    Frame, Terminal,
};

use crate::{
    bootstrap,
    store_args::{build_store, StoreSourceArgs},
    BoxErr,
};

#[derive(Parser)]
#[command(
    name = "mcpstore-tui",
    about = "MCPStore 终端服务管理界面",
    version = env!("CARGO_PKG_VERSION"),
)]
pub struct TuiArgs {
    #[command(flatten)]
    pub store: StoreSourceArgs,
    #[arg(long, default_value_t = 250, help = "事件轮询间隔（毫秒）")]
    pub tick_ms: u64,
}

#[derive(Clone)]
struct ServiceSummary {
    name: String,
    original_name: String,
    agent_id: String,
    transport: String,
    endpoint: String,
    status: ConnectionStatus,
    tools: usize,
    added_time: i64,
}

impl From<ServiceEntry> for ServiceSummary {
    fn from(value: ServiceEntry) -> Self {
        let endpoint = value
            .url
            .clone()
            .or(value.command.clone())
            .unwrap_or_else(|| "-".to_string());

        Self {
            name: value.name,
            original_name: value.original_name,
            agent_id: value.agent_id,
            transport: value.transport,
            endpoint,
            status: value.status,
            tools: value.tools.len(),
            added_time: value.added_time,
        }
    }
}

struct SelectedDetail {
    title: String,
    transport: String,
    endpoint: String,
    scope: String,
    added_time: String,
    connection_status: String,
    health_status: String,
    attempts: String,
    latency: String,
    retry_time: String,
    error_message: String,
    tools: Vec<String>,
}

enum PendingAction {
    Remove(String),
}

struct TuiApp {
    store: mcpstore::store::MCPStore,
    services: Vec<ServiceSummary>,
    selected: usize,
    table_state: TableState,
    selected_detail: Option<SelectedDetail>,
    status_message: String,
    pending_action: Option<PendingAction>,
    should_quit: bool,
    tick_rate: Duration,
    source_label: String,
    backend_label: String,
    namespace: String,
    config_path: String,
}

impl TuiApp {
    fn new(
        store: mcpstore::store::MCPStore,
        tick_rate: Duration,
        source_label: String,
        backend_label: String,
        namespace: String,
        config_path: String,
    ) -> Self {
        Self {
            store,
            services: Vec::new(),
            selected: 0,
            table_state: TableState::default(),
            selected_detail: None,
            status_message: "[进行中] 正在加载服务列表".to_string(),
            pending_action: None,
            should_quit: false,
            tick_rate,
            source_label,
            backend_label,
            namespace,
            config_path,
        }
    }

    fn current_service_name(&self) -> Option<&str> {
        self.services
            .get(self.selected)
            .map(|service| service.name.as_str())
    }

    fn refresh(&mut self, rt: &tokio::runtime::Runtime, reload_source: bool) -> Result<(), BoxErr> {
        if reload_source {
            rt.block_on(async { self.store.load_from_source().await })?;
        }

        let mut services = rt.block_on(async { self.store.list_services().await });
        services.sort_by(|left, right| left.name.cmp(&right.name));

        let selected_name = self.current_service_name().map(ToString::to_string);
        self.services = services.into_iter().map(ServiceSummary::from).collect();

        self.selected = match selected_name {
            Some(name) => self
                .services
                .iter()
                .position(|service| service.name == name)
                .unwrap_or(0),
            None => 0,
        };

        if self.services.is_empty() {
            self.selected = 0;
            self.table_state.select(None);
            self.selected_detail = None;
        } else {
            if self.selected >= self.services.len() {
                self.selected = self.services.len() - 1;
            }
            self.table_state.select(Some(self.selected));
            self.refresh_selected_detail(rt)?;
        }

        Ok(())
    }

    fn refresh_selected_detail(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let Some(name) = self.current_service_name().map(ToString::to_string) else {
            self.selected_detail = None;
            return Ok(());
        };
        let Some(service) = self.services.get(self.selected) else {
            self.selected_detail = None;
            return Ok(());
        };

        let status = match rt.block_on(async { self.store.cached_service_status(&name).await })? {
            Some(status) => Some(status),
            None => rt
                .block_on(async { self.store.health_check(&name).await })
                .ok(),
        };

        let detail = if let Some(status) = status {
            SelectedDetail {
                title: service.name.clone(),
                transport: service.transport.clone(),
                endpoint: service.endpoint.clone(),
                scope: if service.agent_id == "global_agent_store" {
                    "store".to_string()
                } else {
                    format!("agent: {}", service.agent_id)
                },
                added_time: format_timestamp(service.added_time),
                connection_status: format_connection_status(service.status).to_string(),
                health_status: format!("{:?}", status.health_status),
                attempts: format!(
                    "{}/{}",
                    status.connection_attempts, status.max_connection_attempts
                ),
                latency: format_latency(status.latency_p95, status.latency_p99),
                retry_time: status
                    .next_retry_time
                    .map(format_retry_time)
                    .unwrap_or_else(|| "-".to_string()),
                error_message: status.current_error.unwrap_or_else(|| "-".to_string()),
                tools: status
                    .tools
                    .into_iter()
                    .map(|tool| format!("{} [{:?}]", tool.tool_original_name, tool.status))
                    .collect(),
            }
        } else {
            SelectedDetail {
                title: service.name.clone(),
                transport: service.transport.clone(),
                endpoint: service.endpoint.clone(),
                scope: if service.agent_id == "global_agent_store" {
                    "store".to_string()
                } else {
                    format!("agent: {}", service.agent_id)
                },
                added_time: format_timestamp(service.added_time),
                connection_status: format_connection_status(service.status).to_string(),
                health_status: "-".to_string(),
                attempts: "-".to_string(),
                latency: "-".to_string(),
                retry_time: "-".to_string(),
                error_message: "-".to_string(),
                tools: Vec::new(),
            }
        };

        self.selected_detail = Some(detail);
        Ok(())
    }

    fn move_selection(
        &mut self,
        offset: isize,
        rt: &tokio::runtime::Runtime,
    ) -> Result<(), BoxErr> {
        if self.services.is_empty() {
            return Ok(());
        }

        let len = self.services.len() as isize;
        let next = (self.selected as isize + offset).clamp(0, len - 1);
        self.selected = next as usize;
        self.table_state.select(Some(self.selected));
        self.refresh_selected_detail(rt)?;
        Ok(())
    }

    fn jump_to(&mut self, index: usize, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        if self.services.is_empty() {
            return Ok(());
        }
        self.selected = index.min(self.services.len() - 1);
        self.table_state.select(Some(self.selected));
        self.refresh_selected_detail(rt)?;
        Ok(())
    }

    fn handle_key(&mut self, rt: &tokio::runtime::Runtime, key: KeyEvent) -> Result<(), BoxErr> {
        if let Some(action) = self.pending_action.take() {
            return self.handle_pending_action(rt, key, action);
        }

        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1, rt)?,
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1, rt)?,
            KeyCode::Char('g') => self.jump_to(0, rt)?,
            KeyCode::Char('G') => {
                if !self.services.is_empty() {
                    self.jump_to(self.services.len() - 1, rt)?;
                }
            }
            KeyCode::Char('r') => {
                self.refresh(rt, true)?;
                self.status_message = "[成功] 已刷新服务列表".to_string();
            }
            KeyCode::Char('c') => {
                self.connect_selected(rt)?;
            }
            KeyCode::Char('d') => {
                self.disconnect_selected(rt)?;
            }
            KeyCode::Char('x') => {
                self.restart_selected(rt)?;
            }
            KeyCode::Char('D') => {
                if let Some(name) = self.current_service_name().map(ToString::to_string) {
                    self.pending_action = Some(PendingAction::Remove(name.clone()));
                    self.status_message =
                        format!("[警告] 确认删除服务 {name}？按 y 确认，按 n 取消");
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_pending_action(
        &mut self,
        rt: &tokio::runtime::Runtime,
        key: KeyEvent,
        action: PendingAction,
    ) -> Result<(), BoxErr> {
        match key.code {
            KeyCode::Char('y') => match action {
                PendingAction::Remove(name) => {
                    rt.block_on(async { self.store.remove_service(&name).await })?;
                    self.refresh(rt, false)?;
                    self.status_message = format!("[成功] 已删除服务 {name}");
                }
            },
            KeyCode::Char('n') | KeyCode::Esc => {
                self.status_message = "[进行中] 已取消当前操作".to_string();
            }
            _ => {
                self.pending_action = Some(action);
            }
        }
        Ok(())
    }

    fn connect_selected(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let Some(name) = self.current_service_name().map(ToString::to_string) else {
            self.status_message = "[警告] 当前没有可操作的服务".to_string();
            return Ok(());
        };

        rt.block_on(async { self.store.connect_service(&name).await })?;
        self.refresh(rt, false)?;
        self.status_message = format!("[成功] 已连接服务 {name}");
        Ok(())
    }

    fn disconnect_selected(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let Some(name) = self.current_service_name().map(ToString::to_string) else {
            self.status_message = "[警告] 当前没有可操作的服务".to_string();
            return Ok(());
        };

        rt.block_on(async { self.store.disconnect_service(&name).await })?;
        self.refresh(rt, false)?;
        self.status_message = format!("[成功] 已断开服务 {name}");
        Ok(())
    }

    fn restart_selected(&mut self, rt: &tokio::runtime::Runtime) -> Result<(), BoxErr> {
        let Some(name) = self.current_service_name().map(ToString::to_string) else {
            self.status_message = "[警告] 当前没有可操作的服务".to_string();
            return Ok(());
        };

        rt.block_on(async { self.store.restart_service(&name).await })?;
        self.refresh(rt, false)?;
        self.status_message = format!("[成功] 已重启服务 {name}");
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(12),
                Constraint::Length(2),
            ])
            .split(frame.area());

        self.render_header(frame, layout[0]);
        self.render_body(frame, layout[1]);
        self.render_footer(frame, layout[2]);

        if self.pending_action.is_some() {
            self.render_confirm_dialog(frame);
        }
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let service_count = self.services.len();
        let connected = self
            .services
            .iter()
            .filter(|service| service.status == ConnectionStatus::Connected)
            .count();
        let error_count = self
            .services
            .iter()
            .filter(|service| service.status == ConnectionStatus::Error)
            .count();

        let header = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "MCPStore TUI",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  终端服务管理"),
            ]),
            Line::from(format!(
                "source={}  backend={}  namespace={}  services={}  connected={}  error={}",
                self.source_label,
                self.backend_label,
                self.namespace,
                service_count,
                connected,
                error_count
            )),
            Line::from(format!("config={}", self.config_path)),
        ])
        .block(Block::default().borders(Borders::ALL).title("概览"));

        frame.render_widget(header, area);
    }

    fn render_body(&mut self, frame: &mut Frame, area: Rect) {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
            .split(area);

        self.render_service_table(frame, columns[0]);
        self.render_detail_panel(frame, columns[1]);
    }

    fn render_service_table(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(vec!["名称", "作用域", "协议", "状态", "工具"])
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .height(1);

        let rows = self.services.iter().map(|service| {
            let scope = if service.agent_id == "global_agent_store" {
                "store".to_string()
            } else {
                truncate_text(&service.agent_id, 10)
            };

            Row::new(vec![
                truncate_text(&service.name, 22),
                scope,
                truncate_text(&service.transport, 10),
                format_connection_status(service.status).to_string(),
                service.tools.to_string(),
            ])
            .style(status_style(service.status))
        });

        let table = Table::new(
            rows,
            [
                Constraint::Length(24),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(12),
                Constraint::Length(6),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("服务列表"))
        .row_highlight_style(
            Style::default()
                .bg(Color::Rgb(32, 42, 54))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

        frame.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_detail_panel(&self, frame: &mut Frame, area: Rect) {
        let lines = if let Some(detail) = &self.selected_detail {
            let mut lines = vec![
                Line::from(Span::styled(
                    detail.title.clone(),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(format!("协议: {}", detail.transport)),
                Line::from(format!("入口: {}", detail.endpoint)),
                Line::from(format!("作用域: {}", detail.scope)),
                Line::from(format!(
                    "原始名称: {}",
                    self.services[self.selected].original_name
                )),
                Line::from(format!("加入时间: {}", detail.added_time)),
                Line::from(format!("连接状态: {}", detail.connection_status)),
                Line::from(format!("健康状态: {}", detail.health_status)),
                Line::from(format!("重试次数: {}", detail.attempts)),
                Line::from(format!("延迟: {}", detail.latency)),
                Line::from(format!("下次重试: {}", detail.retry_time)),
                Line::from(format!("错误: {}", detail.error_message)),
                Line::from(""),
                Line::from(Span::styled(
                    "工具状态",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
            ];

            if detail.tools.is_empty() {
                lines.push(Line::from("- 暂无工具"));
            } else {
                for tool in detail.tools.iter().take(12) {
                    lines.push(Line::from(format!("- {}", truncate_text(tool, 72))));
                }
                if detail.tools.len() > 12 {
                    lines.push(Line::from(format!(
                        "- 其余 {} 个工具未展开",
                        detail.tools.len() - 12
                    )));
                }
            }

            lines
        } else {
            vec![
                Line::from("暂无服务"),
                Line::from("使用左侧列表查看当前可管理的 MCP 服务。"),
            ]
        };

        let panel = Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("详情"))
            .wrap(Wrap { trim: true });

        frame.render_widget(panel, area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let footer = Paragraph::new(vec![
            Line::from(self.status_message.clone()),
            Line::from(
                "按键: ↑/k ↓/j 移动  g/G 跳转  c 连接  d 断开  x 重启  D 删除  r 刷新  q 退出",
            ),
        ])
        .block(Block::default().borders(Borders::ALL).title("状态"));

        frame.render_widget(footer, area);
    }

    fn render_confirm_dialog(&self, frame: &mut Frame) {
        let area = centered_rect(48, 20, frame.area());
        frame.render_widget(Clear, area);

        let text = Paragraph::new(vec![
            Line::from(Span::styled(
                "确认删除",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("删除会同步修改当前 store 配置与运行态缓存。"),
            Line::from("按 y 确认删除，按 n 或 Esc 取消。"),
        ])
        .block(Block::default().borders(Borders::ALL).title("危险操作"))
        .wrap(Wrap { trim: true });

        frame.render_widget(text, area);
    }
}

struct TerminalGuard;

impl TerminalGuard {
    fn enter(stdout: &mut Stdout) -> io::Result<Self> {
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        disable_raw_mode().ok();
        let mut stdout = io::stdout();
        execute!(stdout, LeaveAlternateScreen).ok();
    }
}

pub fn run() -> Result<(), BoxErr> {
    bootstrap::init_tracing("mcpstore=warn");

    let args = TuiArgs::parse();
    let rt = bootstrap::build_runtime()?;
    let store = build_store(&args.store)?;
    rt.block_on(async { store.load_from_source().await })?;

    let source_label = args.store.source.as_str().to_string();
    let backend_label = rt.block_on(async { store.current_backend().await.as_str().to_string() });
    let namespace = store.namespace();
    let config_path = store.config_manager().mcp_path().display().to_string();

    let mut app = TuiApp::new(
        store,
        Duration::from_millis(args.tick_ms),
        source_label,
        backend_label,
        namespace,
        config_path,
    );
    app.refresh(&rt, false)?;
    app.status_message = "[成功] TUI 已就绪，按 q 退出".to_string();

    let mut stdout = io::stdout();
    let _guard = TerminalGuard::enter(&mut stdout)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|frame| app.render(frame))?;
        if app.should_quit {
            break;
        }

        if event::poll(app.tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                    if let Err(error) = app.handle_key(&rt, key) {
                        app.status_message = format!("[错误] {error}");
                    }
                }
            }
        }
    }

    terminal.show_cursor()?;
    Ok(())
}

fn status_style(status: ConnectionStatus) -> Style {
    match status {
        ConnectionStatus::Connected => Style::default().fg(Color::Green),
        ConnectionStatus::Connecting => Style::default().fg(Color::Yellow),
        ConnectionStatus::Disconnected => Style::default().fg(Color::Gray),
        ConnectionStatus::Error => Style::default().fg(Color::Red),
    }
}

fn format_connection_status(status: ConnectionStatus) -> &'static str {
    match status {
        ConnectionStatus::Connected => "connected",
        ConnectionStatus::Connecting => "connecting",
        ConnectionStatus::Disconnected => "disconnected",
        ConnectionStatus::Error => "error",
    }
}

fn truncate_text(value: &str, limit: usize) -> String {
    let mut chars = value.chars();
    let head: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{head}...")
    } else {
        head
    }
}

fn format_latency(p95: Option<f64>, p99: Option<f64>) -> String {
    match (p95, p99) {
        (Some(p95), Some(p99)) => format!("p95={p95:.0}ms  p99={p99:.0}ms"),
        _ => "-".to_string(),
    }
}

fn format_retry_time(timestamp: f64) -> String {
    let seconds = timestamp.trunc() as i64;
    let fractional = (timestamp.fract() * 1_000_000_000.0) as u32;
    chrono::DateTime::from_timestamp(seconds, fractional)
        .map(|time| time.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

fn format_timestamp(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "-".to_string();
    }

    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|time| time.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| timestamp.to_string())
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use mcpstore::config::ServerConfig;
    use mcpstore::store::MCPStore;
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_config_path() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("mcpstore-tui-test-{nanos}.json"))
            .to_string_lossy()
            .to_string()
    }

    fn stdio_config() -> ServerConfig {
        ServerConfig {
            url: None,
            command: Some("echo".to_string()),
            args: vec!["fixture".to_string()],
            env: HashMap::new(),
            headers: HashMap::new(),
            transport: Some("stdio".to_string()),
            working_dir: None,
            description: Some("fixture".to_string()),
        }
    }

    #[test]
    fn parses_tui_args() {
        let args = TuiArgs::try_parse_from([
            "mcpstore-tui",
            "--source",
            "db",
            "--backend",
            "redis",
            "--redis-url",
            "redis://127.0.0.1:6379/0",
            "--namespace",
            "demo",
            "--tick-ms",
            "500",
        ])
        .unwrap();

        assert_eq!(args.store.source, crate::store_args::SourceArg::Db);
        assert_eq!(
            args.store.backend,
            Some(crate::store_args::BackendArg::Redis)
        );
        assert_eq!(args.store.namespace.as_deref(), Some("demo"));
        assert_eq!(args.tick_ms, 500);
    }

    #[test]
    fn refresh_loads_service_detail_into_app() {
        let path = temp_config_path();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let store = MCPStore::setup(Some(&path)).unwrap();
        rt.block_on(store.add_service("svc", stdio_config()))
            .unwrap();

        let mut app = TuiApp::new(
            store,
            Duration::from_millis(100),
            "local".to_string(),
            "memory".to_string(),
            "mcpstore".to_string(),
            path.clone(),
        );
        app.refresh(&rt, false).unwrap();

        assert_eq!(app.services.len(), 1);
        assert_eq!(app.services[0].name, "svc");
        let detail = app.selected_detail.as_ref().unwrap();
        assert_eq!(detail.title, "svc");
        assert_eq!(detail.transport, "stdio");

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn handle_quit_key_marks_app_for_exit() {
        let path = temp_config_path();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let store = MCPStore::setup(Some(&path)).unwrap();
        let mut app = TuiApp::new(
            store,
            Duration::from_millis(100),
            "local".to_string(),
            "memory".to_string(),
            "mcpstore".to_string(),
            path.clone(),
        );

        app.handle_key(&rt, KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE))
            .unwrap();

        assert!(app.should_quit);
        std::fs::remove_file(path).ok();
    }
}
