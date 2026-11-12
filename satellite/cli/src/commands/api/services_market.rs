//! MCP services market: a curated featured list compiled into the binary,
//! then the official MCP Registry (registry.modelcontextprotocol.io) for
//! search and endless scroll. Install goes through the existing
//! POST /services/:name — every item carries a ready `ServerConfig` payload,
//! with secret/argument values collected from the user at install time.
//!
//! The algorithms (address normalisation, ranking, registry conversion,
//! server-key identity) are ported from magpie's `internal/library/market.go`
//! / `market_addr.go`; see `local/调研文档-mcp市场调研v1.md` for the design.
//! Deliberately not ported: the icon proxy (the browser loads icons
//! directly), skills market, and the add-custom suggestion (the panel's
//! add-service dialog already covers manual entries).

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant},
};

use axum::extract::Query;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::envelope::{success, ApiResult};

const REGISTRY_URL: &str = "https://registry.modelcontextprotocol.io/v0/servers";

/// The registry answers slowly often enough (13-37 s, magpie #3043) that the
/// shared HTTP client's budget is too tight; its asks get their own client.
static REGISTRY_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .connect_timeout(Duration::from_secs(10))
        .build()
        .expect("registry client configuration is valid")
});

/// One value a service needs from the user before it can start: a key in its
/// environment, a header it sends, or an argument.
#[derive(Debug, Clone, Serialize)]
pub struct MarketInput {
    pub key: String,
    /// `env`, `header` or `arg`.
    pub r#where: String,
    pub label: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub placeholder: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub secret: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub required: bool,
    /// A header value's template around the input, `"Bearer {}"`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub format: String,
}

/// One service the market offers, installable by POSTing `config` (with the
/// user's input values filled in) to `/services/:name`.
#[derive(Debug, Clone, Serialize)]
pub struct MarketService {
    pub id: String,
    /// The name the service is installed under.
    pub name: String,
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub publisher: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub icon: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub homepage: String,
    pub transport: String,
    /// A remote one that has the user sign in, in the browser, the first
    /// time it's used.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub sign_in: bool,
    /// Found at the address that was searched for.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub matched: bool,
    /// The mcpstore `ServerConfig` payload, without the input values.
    pub config: Value,
    pub inputs: Vec<MarketInput>,
}

impl MarketService {
    fn server_key(&self) -> String {
        server_key(&self.config)
    }
}

fn github_icon(owner: &str) -> String {
    format!("https://github.com/{owner}.png?size=96")
}

fn secret_env(key: &str, label: &str, description: &str) -> MarketInput {
    MarketInput {
        key: key.to_string(),
        r#where: "env".into(),
        label: label.into(),
        description: description.into(),
        placeholder: String::new(),
        secret: true,
        required: true,
        format: String::new(),
    }
}

fn remote(
    id: &str,
    publisher: &str,
    icon: &str,
    homepage: &str,
    url: &str,
    description: &str,
    sign_in: bool,
) -> MarketService {
    MarketService {
        id: id.into(),
        name: id.into(),
        title: id.into(),
        publisher: publisher.into(),
        description: description.into(),
        icon: icon.into(),
        homepage: homepage.into(),
        transport: "streamable-http".into(),
        sign_in,
        matched: false,
        config: json!({ "url": url, "transport": "streamable-http" }),
        inputs: Vec::new(),
    }
}

fn local(
    id: &str,
    publisher: &str,
    icon: &str,
    homepage: &str,
    description: &str,
    command: &str,
    args: &[&str],
) -> MarketService {
    MarketService {
        id: id.into(),
        name: id.into(),
        title: id.into(),
        publisher: publisher.into(),
        description: description.into(),
        icon: icon.into(),
        homepage: homepage.into(),
        transport: "stdio".into(),
        sign_in: false,
        matched: false,
        config: json!({
            "command": command,
            "args": args,
            "transport": "stdio",
        }),
        inputs: Vec::new(),
    }
}

fn header_input(
    key: &str,
    label: &str,
    description: &str,
    placeholder: &str,
    format: &str,
) -> MarketInput {
    MarketInput {
        key: key.into(),
        r#where: "header".into(),
        label: label.into(),
        description: description.into(),
        placeholder: placeholder.into(),
        secret: true,
        required: true,
        format: format.into(),
    }
}

fn arg_input(label: &str, description: &str, placeholder: &str) -> MarketInput {
    MarketInput {
        key: "arg0".into(),
        r#where: "arg".into(),
        label: label.into(),
        description: description.into(),
        placeholder: placeholder.into(),
        secret: false,
        required: true,
        format: String::new(),
    }
}

/// The services the market offers first. Every one was checked: its package
/// is published, its endpoint answers. Titles are set from the magpie
/// catalogue the panel's Services tab is modelled on.
static FEATURED: LazyLock<Vec<MarketService>> = LazyLock::new(|| {
    let mut list = vec![
        remote("context7", "Upstash", &github_icon("upstash"), "https://context7.com",
            "https://mcp.context7.com/mcp",
            "Up-to-date documentation and code examples for any library, straight into the prompt.", false),
        local("playwright", "Microsoft", "https://playwright.dev/img/playwright-logo.svg",
            "https://github.com/microsoft/playwright-mcp",
            "Drive a real browser: open pages, click, type and read them through the accessibility tree.",
            "npx", &["-y", "@playwright/mcp@latest"]),
        local("chrome-devtools", "Google", &github_icon("ChromeDevTools"),
            "https://github.com/ChromeDevTools/chrome-devtools-mcp",
            "Control and inspect a live Chrome: console, network, performance traces and screenshots.",
            "npx", &["-y", "chrome-devtools-mcp@latest"]),
        remote("github", "GitHub", &github_icon("github"), "https://github.com/github/github-mcp-server",
            "https://api.githubcopilot.com/mcp/",
            "Repositories, issues, pull requests, Actions and code search on GitHub.", false),
        remote("deepwiki", "Cognition", &github_icon("CognitionAI"), "https://deepwiki.com",
            "https://mcp.deepwiki.com/mcp",
            "Ask questions about any public GitHub repository and read its generated wiki.", false),
        remote("exa", "Exa", &github_icon("exa-labs"), "https://docs.exa.ai/reference/exa-mcp",
            "https://mcp.exa.ai/mcp",
            "Web search and code search built for agents, with clean page contents.", false),
        local("filesystem", "Model Context Protocol", &github_icon("modelcontextprotocol"),
            "https://github.com/modelcontextprotocol/servers/tree/main/src/filesystem",
            "Read, write and search files, limited to the folders you allow.",
            "npx", &["-y", "@modelcontextprotocol/server-filesystem"]),
        local("fetch", "Model Context Protocol", &github_icon("modelcontextprotocol"),
            "https://github.com/modelcontextprotocol/servers/tree/main/src/fetch",
            "Fetch a web page and hand it over as Markdown.", "uvx", &["mcp-server-fetch"]),
        local("memory", "Model Context Protocol", &github_icon("modelcontextprotocol"),
            "https://github.com/modelcontextprotocol/servers/tree/main/src/memory",
            "A knowledge graph the agent keeps across conversations.",
            "npx", &["-y", "@modelcontextprotocol/server-memory"]),
        local("sequential-thinking", "Model Context Protocol", &github_icon("modelcontextprotocol"),
            "https://github.com/modelcontextprotocol/servers/tree/main/src/sequentialthinking",
            "Think a problem through step by step, revising and branching as it goes.",
            "npx", &["-y", "@modelcontextprotocol/server-sequential-thinking"]),
        local("git", "Model Context Protocol", &github_icon("modelcontextprotocol"),
            "https://github.com/modelcontextprotocol/servers/tree/main/src/git",
            "Read, search and change Git repositories.", "uvx", &["mcp-server-git"]),
        local("time", "Model Context Protocol", &github_icon("modelcontextprotocol"),
            "https://github.com/modelcontextprotocol/servers/tree/main/src/time",
            "The current time anywhere, and conversions between time zones.",
            "uvx", &["mcp-server-time"]),
        remote("sentry", "Sentry", &github_icon("getsentry"), "https://docs.sentry.io/product/sentry-mcp/",
            "https://mcp.sentry.dev/mcp", "Issues, errors and traces from Sentry, and Seer's analysis of them.", true),
        remote("linear", "Linear", &github_icon("linear"), "https://linear.app/docs/mcp",
            "https://mcp.linear.app/mcp", "Find, create and update Linear issues, projects and comments.", true),
        remote("notion", "Notion", &github_icon("makenotion"), "https://developers.notion.com/docs/mcp",
            "https://mcp.notion.com/mcp", "Search, read and write pages and databases in your Notion workspace.", true),
        remote("atlassian", "Atlassian", &github_icon("atlassian"), "https://www.atlassian.com/platform/remote-mcp-server",
            "https://mcp.atlassian.com/v1/mcp", "Jira issues and Confluence pages from Atlassian Cloud.", true),
        remote("supabase", "Supabase", &github_icon("supabase"), "https://supabase.com/docs/guides/getting-started/mcp",
            "https://mcp.supabase.com/mcp", "Manage Supabase projects: tables, SQL, migrations, logs and edge functions.", true),
        remote("stripe", "Stripe", &github_icon("stripe"), "https://docs.stripe.com/mcp",
            "https://mcp.stripe.com", "Customers, payments, subscriptions and Stripe's documentation.", true),
        remote("vercel", "Vercel", &github_icon("vercel"), "https://vercel.com/docs/mcp/vercel-mcp",
            "https://mcp.vercel.com", "Projects, deployments and logs on Vercel, and its documentation.", true),
        remote("neon", "Neon", &github_icon("neondatabase"), "https://neon.com/docs/ai/neon-mcp-server",
            "https://mcp.neon.tech/mcp", "Serverless Postgres on Neon: projects, branches and SQL.", true),
        remote("cloudflare-docs", "Cloudflare", &github_icon("cloudflare"), "https://github.com/cloudflare/mcp-server-cloudflare",
            "https://docs.mcp.cloudflare.com/mcp", "Search Cloudflare's documentation.", false),
        remote("microsoft-learn", "Microsoft", &github_icon("microsoft"), "https://github.com/MicrosoftDocs/mcp",
            "https://learn.microsoft.com/api/mcp", "Official Microsoft and Azure documentation and code samples.", false),
        remote("huggingface", "Hugging Face", &github_icon("huggingface"), "https://huggingface.co/settings/mcp",
            "https://huggingface.co/mcp", "Search models, datasets, Spaces and papers on the Hugging Face Hub.", false),
        local("brave-search", "Brave", &github_icon("brave"), "https://github.com/brave/brave-search-mcp-server",
            "Web, news, image and local search from Brave's independent index.",
            "npx", &["-y", "@brave/brave-search-mcp-server"]),
        local("firecrawl", "Firecrawl", &github_icon("firecrawl"), "https://github.com/firecrawl/firecrawl-mcp-server",
            "Scrape, crawl and search the web into clean Markdown.", "npx", &["-y", "firecrawl-mcp"]),
        local("tavily", "Tavily", &github_icon("tavily-ai"), "https://github.com/tavily-ai/tavily-mcp",
            "Search and extract from the web with Tavily.", "npx", &["-y", "tavily-mcp"]),
    ];
    // Inputs are attached after construction so each entry stays one line.
    let mut inputs: HashMap<&str, Vec<MarketInput>> = HashMap::new();
    inputs.insert(
        "context7",
        vec![MarketInput {
            key: "CONTEXT7_API_KEY".into(),
            r#where: "header".into(),
            label: "API key".into(),
            description: "Optional — raises the rate limit. From context7.com/dashboard".into(),
            placeholder: String::new(),
            secret: true,
            required: false,
            format: "{}".into(),
        }],
    );
    inputs.insert(
        "github",
        vec![header_input(
            "Authorization",
            "Personal access token",
            "github.com/settings/personal-access-tokens",
            "github_pat_…",
            "Bearer {}",
        )],
    );
    inputs.insert(
        "filesystem",
        vec![arg_input(
            "Folder",
            "The folder it may read and write",
            "~/code",
        )],
    );
    inputs.insert(
        "brave-search",
        vec![secret_env(
            "BRAVE_API_KEY",
            "API key",
            "brave.com/search/api",
        )],
    );
    inputs.insert(
        "firecrawl",
        vec![secret_env(
            "FIRECRAWL_API_KEY",
            "API key",
            "firecrawl.dev/app/api-keys",
        )],
    );
    inputs.insert(
        "tavily",
        vec![secret_env("TAVILY_API_KEY", "API key", "app.tavily.com")],
    );
    let mut titles: HashMap<&str, &str> = HashMap::new();
    for (id, title) in [
        ("context7", "Context7"),
        ("playwright", "Playwright"),
        ("chrome-devtools", "Chrome DevTools"),
        ("github", "GitHub"),
        ("deepwiki", "DeepWiki"),
        ("exa", "Exa"),
        ("filesystem", "Filesystem"),
        ("fetch", "Fetch"),
        ("memory", "Memory"),
        ("sequential-thinking", "Sequential Thinking"),
        ("git", "Git"),
        ("time", "Time"),
        ("sentry", "Sentry"),
        ("linear", "Linear"),
        ("notion", "Notion"),
        ("atlassian", "Atlassian"),
        ("supabase", "Supabase"),
        ("stripe", "Stripe"),
        ("vercel", "Vercel"),
        ("neon", "Neon"),
        ("cloudflare-docs", "Cloudflare Docs"),
        ("microsoft-learn", "Microsoft Learn"),
        ("huggingface", "Hugging Face"),
        ("brave-search", "Brave Search"),
        ("firecrawl", "Firecrawl"),
        ("tavily", "Tavily"),
    ] {
        titles.insert(id, title);
    }
    for service in &mut list {
        if let Some(values) = inputs.get(service.id.as_str()) {
            service.inputs = values.clone();
        }
        if let Some(title) = titles.get(service.id.as_str()) {
            service.title = (*title).to_string();
        }
    }
    list
});

/// What a featured service is, wherever it may be listed under another name:
/// its URL, or the command and the package it starts. Registry entries whose
/// key matches a featured one are dropped, so the catalogue never shows a
/// service twice.
static FEATURED_KEYS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    FEATURED
        .iter()
        .map(|service| service.server_key())
        .collect()
});

/// What the registry listed for a query, kept briefly: one fetch answers
/// every page of that query's endless scroll.
static REGISTRY_CACHE: LazyLock<Mutex<HashMap<String, CacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct CacheEntry {
    at: Instant,
    items: std::sync::Arc<Vec<MarketService>>,
}

const CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const CACHE_CAP: usize = 32;

// ---- address handling (ported from market_addr.go) --------------------------

/// Hosts that keep repositories and images under owner/name.
const CODE_HOSTS: [&str; 8] = [
    "github.com",
    "gitlab.com",
    "bitbucket.org",
    "codeberg.org",
    "gitee.com",
    "ghcr.io",
    "docker.io",
    "quay.io",
];

/// Where `://` starts in `s`, when its scheme reads as one.
fn scheme_start(s: &str) -> Option<usize> {
    let pos = s.find("://")?;
    let scheme = &s[..pos];
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_lowercase() {
        return None;
    }
    chars
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '+' || c == '.' || c == '-')
        .then_some(pos)
}

/// An address as it's compared: lower case, with no scheme, user, www.,
/// query, fragment, trailing slash or .git; a package's page is the package,
/// and an image has no tag.
fn norm_addr(s: &str) -> String {
    let mut a = s.trim().to_lowercase();
    if a.is_empty() {
        return a;
    }
    if let Some(rest) = a.strip_prefix("git@") {
        a = rest.replacen(':', "/", 1);
    }
    if let Some(pos) = scheme_start(&a) {
        let rest = a[pos + 3..].to_string();
        let (host, path) = match rest.split_once('/') {
            Some((host, path)) => (host, Some(path)),
            None => (rest.as_str(), None),
        };
        let host = host.split_once('@').map(|(_, h)| h).unwrap_or(host);
        a = match path {
            Some(path) => format!("{host}/{path}"),
            None => host.to_string(),
        };
    }
    if let Some(i) = a.find(['?', '#']) {
        a.truncate(i);
    }
    a = a
        .strip_prefix("www.")
        .unwrap_or(&a)
        .trim_end_matches('/')
        .to_string();
    a = a.strip_suffix(".git").unwrap_or(&a).to_string();
    for prefix in [
        "npmjs.com/package/",
        "npmjs.org/package/",
        "pypi.org/project/",
        "hub.docker.com/r/",
        "hub.docker.com/_/",
        "docker.io/library/",
        "docker.io/",
        "registry.hub.docker.com/r/",
    ] {
        if let Some(rest) = a.strip_prefix(prefix) {
            a = rest.to_string();
            break;
        }
    }
    // A package's version or an image's tag: @scope/pkg@1.2, pkg@latest, img:tag
    let slash = a.rfind('/').map(|i| i + 1).unwrap_or(0);
    let last = &a[slash..];
    let mut cut = a.len();
    if let Some(i) = last.rfind('@') {
        if i > 0 {
            cut = slash + i;
        }
    }
    if cut == a.len() {
        if let Some(i) = last.rfind(':') {
            if i > 0 && a.contains('/') {
                cut = slash + i;
            }
        }
    }
    a.truncate(cut);
    a.trim_end_matches('/').to_string()
}

fn has_scheme(s: &str) -> bool {
    scheme_start(s).is_some()
}

/// A host with at least two labels, optionally ending in :port.
fn is_host(s: &str) -> bool {
    let s = s.split_once(':').map_or(s, |(h, _)| h);
    let labels: Vec<&str> = s.split('.').collect();
    labels.len() >= 2
        && labels.iter().all(|l| {
            !l.is_empty()
                && l.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
}

/// `owner/repo` or `@scope/pkg`.
fn is_owner_repo(s: &str) -> bool {
    let s = s.strip_prefix('@').unwrap_or(s);
    let Some((owner, repo)) = s.split_once('/') else {
        return false;
    };
    let ok = |part: &str| {
        !part.is_empty()
            && part.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.' || c == '-'
            })
    };
    let first_ok = owner
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    first_ok && ok(owner) && ok(repo)
}

/// A search read as an address, normalised; `None` when it reads as words to
/// look for.
fn address(q: &str) -> Option<String> {
    let trimmed = q.trim();
    if trimmed.is_empty() || trimmed.contains([' ', '\t', '\n']) {
        return None;
    }
    let low = trimmed.to_lowercase();
    let a = norm_addr(trimmed);
    if a.is_empty() {
        return None;
    }
    let host = a.split('/').next().unwrap_or("");
    let has_path = a.contains('/');
    if has_scheme(&low)
        || low.starts_with("git@")
        || low.starts_with("www.")
        || (has_path && is_host(host))
        || is_owner_repo(&a)
    {
        Some(a)
    } else {
        None
    }
}

/// An address's host, when it has one.
fn host_of(a: &str) -> &str {
    let host = a.split('/').next().unwrap_or("");
    if a.starts_with('@') || !host.contains('.') {
        ""
    } else {
        host
    }
}

/// What a service is, whatever it's called: its URL, or the command and the
/// package it starts.
fn server_key(config: &Value) -> String {
    if let Some(url) = config.get("url").and_then(Value::as_str) {
        return url.trim_end_matches('/').to_lowercase();
    }
    let command = config.get("command").and_then(Value::as_str).unwrap_or("");
    let args: Vec<&str> = config
        .get("args")
        .and_then(Value::as_array)
        .map(|args| args.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    for arg in args {
        if arg.is_empty() || arg.starts_with('-') || arg == "run" {
            continue;
        }
        let pkg = match arg.rfind('@') {
            Some(i) if i > 0 => &arg[..i],
            _ => arg,
        };
        return format!("{command} {pkg}");
    }
    command.to_string()
}

/// How well an address searched for is one of a service's: 3 it is, 2 it is
/// under it (a repository for a server in one of its folders), 1 the
/// service's is under it (a file in its repository); 0 neither. One with no
/// host (owner/repo) is matched on any host.
fn addr_score(q: &str, addrs: &[String]) -> usize {
    let mut best = 0;
    for e in addrs {
        let mut candidates: Vec<&str> = vec![e.as_str()];
        if host_of(q).is_empty() && !host_of(e).is_empty() {
            candidates.push(e.split_once('/').map(|(_, rest)| rest).unwrap_or(""));
        }
        for c in candidates {
            if c.is_empty() {
                continue;
            }
            let mut depth = c.matches('/').count();
            if host_of(c).is_empty() {
                depth += 1; // owner/repo is as deep as github.com/owner/repo
            }
            if c == q {
                best = best.max(3);
            } else if c.strip_prefix(q).is_some_and(|rest| rest.starts_with('/')) {
                best = best.max(2);
            } else if depth >= 2 && q.strip_prefix(c).is_some_and(|rest| rest.starts_with('/')) {
                best = best.max(1);
            }
        }
    }
    best
}

/// What a host is called: example for mcp.example.com.
fn site_name(host: &str) -> String {
    let host = host.split_once(':').map_or(host, |(h, _)| h);
    let mut labels: Vec<&str> = host.split('.').collect();
    if labels.len() > 1 {
        labels.pop(); // the top-level domain
    }
    for label in labels.iter().rev() {
        if matches!(
            *label,
            "www" | "mcp" | "api" | "app" | "docs" | "server" | "co" | "com"
        ) {
            continue;
        }
        return (*label).to_string();
    }
    String::new()
}

/// The names the registry is asked for to find the services at an address: a
/// repository's or package's name and then its owner's, or a site's name
/// (context7 for mcp.context7.com/mcp). At most two.
fn addr_terms(a: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    let add = |t: &str, terms: &mut Vec<String>| {
        let t = t.trim_start_matches('@');
        if t.len() >= 2 && !terms.iter().any(|x| x == t) && terms.len() < 2 {
            terms.push(t.to_string());
        }
    };
    let host = host_of(a);
    let path = if host.is_empty() {
        a
    } else {
        a.split_once('/').map(|(_, p)| p).unwrap_or("")
    };
    let parts: Vec<&str> = path.split('/').collect();
    if host.is_empty() || CODE_HOSTS.contains(&host) {
        if parts.len() >= 2 {
            add(parts[1], &mut terms);
        }
        add(parts[0], &mut terms);
    } else {
        add(&site_name(host), &mut terms);
        add(parts[parts.len() - 1], &mut terms);
    }
    terms
}

/// A registry name as the library would call it:
/// io.github.upstash/context7-mcp is context7.
fn short_name(name: &str) -> String {
    let mut n = name[name.rfind('/').map(|i| i + 1).unwrap_or(0)..].to_lowercase();
    let orig = n.clone();
    for prefix in ["mcp-server-", "server-", "mcp-"] {
        if let Some(rest) = n.strip_prefix(prefix) {
            n = rest.to_string();
        }
    }
    for suffix in ["-mcp-server", "-mcp", "-server", "_mcp", ".mcp"] {
        if let Some(rest) = n.strip_suffix(suffix) {
            n = rest.to_string();
        }
    }
    if n.is_empty() {
        n = orig;
    }
    let mut cleaned: String = n
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    cleaned = cleaned.trim_matches(|c| c == '-' || c == '_').to_string();
    if cleaned.len() > 64 {
        cleaned = cleaned[..64].to_string();
    }
    if cleaned.is_empty()
        || !cleaned
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        "server".to_string()
    } else {
        cleaned
    }
}

fn github_owner(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| url.strip_prefix("https://www.github.com/"))?;
    let owner = rest.trim_matches('/').split('/').next()?;
    (!owner.is_empty()).then(|| owner.to_string())
}

// ---- the registry -----------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RegistryResponse {
    #[serde(default)]
    servers: Vec<RegistryEntry>,
}

#[derive(Debug, Deserialize)]
struct RegistryEntry {
    #[serde(default)]
    server: RegistryServer,
    #[serde(default)]
    _meta: HashMap<String, RegistryMeta>,
}

#[derive(Debug, Deserialize)]
struct RegistryMeta {
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RegistryServer {
    #[serde(default)]
    name: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "websiteUrl")]
    website_url: Option<String>,
    #[serde(default)]
    repository: Option<RegistryRepo>,
    #[serde(default)]
    icons: Vec<RegistryIcon>,
    #[serde(default)]
    remotes: Vec<RegistryRemote>,
    #[serde(default)]
    packages: Vec<RegistryPackage>,
}

#[derive(Debug, Deserialize)]
struct RegistryRepo {
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegistryIcon {
    #[serde(default)]
    src: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegistryRemote {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    headers: Vec<RegistryVar>,
}

#[derive(Debug, Deserialize)]
struct RegistryPackage {
    #[serde(default, rename = "registryType")]
    registry_type: Option<String>,
    #[serde(default)]
    identifier: Option<String>,
    #[serde(default)]
    transport: Option<RegistryPackageTransport>,
    #[serde(default, rename = "environmentVariables")]
    environment_variables: Vec<RegistryVar>,
    #[serde(default, rename = "packageArguments")]
    package_arguments: Vec<RegistryArgument>,
}

#[derive(Debug, Deserialize)]
struct RegistryPackageTransport {
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegistryVar {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "isRequired")]
    is_required: bool,
    #[serde(default, rename = "isSecret")]
    is_secret: bool,
}

#[derive(Debug, Deserialize)]
struct RegistryArgument {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    default: Option<String>,
    #[serde(default, rename = "valueHint")]
    value_hint: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "isRequired")]
    is_required: bool,
}

/// A `{placeholder}` in a registry value: any `{...}` span.
fn has_placeholder(s: &str) -> bool {
    let bytes = s.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'{' && bytes[i + 1..].iter().any(|b| *b == b'}') {
            return true;
        }
    }
    false
}

/// How well a registry entry reads as what was searched for: what's called
/// what was searched for first, then what says it is, with an icon and a
/// repository. Ported from magpie's rank().
fn rank(r: &RegistryServer, m: &MarketService, q: &str) -> i32 {
    let mut s = 0;
    let short = r.name.rsplit('/').next().unwrap_or("").to_lowercase();
    if m.name == q
        || r.title.as_deref().map(|t| t.to_lowercase()) == Some(q.to_string())
        || short == q
    {
        s += 100;
    } else if short.contains(q) {
        s += 40;
    }
    if r.title
        .as_deref()
        .map(|t| t.to_lowercase())
        .is_some_and(|t| t.contains(q))
    {
        s += 20;
    }
    if r.description
        .as_deref()
        .map(|d| d.to_lowercase())
        .is_some_and(|d| d.contains(q))
    {
        s += 10;
    }
    if !r.icons.is_empty() {
        s += 8;
    }
    if r.repository
        .as_ref()
        .and_then(|repo| repo.url.as_deref())
        .is_some_and(|u| !u.is_empty())
    {
        s += 5;
    }
    if r.name.starts_with("ai.smithery/") {
        s -= 60; // wrappers around others' servers
    }
    s
}

/// What a registry entry becomes in the market: its remote when it has one
/// the store can speak (streamable-http; sse is not a store transport), else
/// the package it's published as. `None` for entries the market can't fill a
/// working configuration for.
fn from_registry(r: &RegistryServer) -> Option<MarketService> {
    let name = short_name(&r.name);
    let repo_url = r
        .repository
        .as_ref()
        .and_then(|repo| repo.url.as_deref())
        .unwrap_or("");
    let mut publisher = github_owner(repo_url);
    if publisher.is_none() {
        if let Some(rest) = r.name.strip_prefix("io.github.") {
            publisher = rest.split('/').next().map(|owner| owner.to_string());
        }
    }
    let publisher = publisher.unwrap_or_else(|| {
        let namespace = r.name.split('/').next().unwrap_or("");
        let mut parts: Vec<&str> = namespace.split('.').collect();
        parts.reverse();
        parts.join(".")
    });
    let mut service = MarketService {
        id: r.name.clone(),
        name,
        title: r.title.clone().unwrap_or_default(),
        publisher,
        description: r.description.clone().unwrap_or_default(),
        icon: String::new(),
        homepage: r.website_url.clone().unwrap_or_default(),
        transport: String::new(),
        sign_in: false,
        matched: false,
        config: json!({}),
        inputs: Vec::new(),
    };
    if service.title.is_empty() {
        service.title = service.name.clone();
    }
    if service.homepage.is_empty() {
        service.homepage = repo_url.to_string();
    }
    service.icon = r
        .icons
        .iter()
        .find_map(|icon| {
            icon.src
                .as_deref()
                .filter(|src| src.starts_with("https://"))
        })
        .map(str::to_string)
        .unwrap_or_else(|| {
            service
                .publisher
                .split('/')
                .next()
                .filter(|owner| !owner.is_empty() && !owner.contains('.'))
                .map(github_icon)
                .unwrap_or_default()
        });

    let mut addrs: Vec<String> = Vec::new();
    for candidate in [
        Some(r.name.as_str()),
        r.website_url.as_deref(),
        Some(repo_url),
    ] {
        push_addr(&mut addrs, candidate);
    }

    // A remote the store speaks: streamable-http only (the store's validator
    // rejects sse), https, with no unfilled placeholder in the URL.
    for rm in &r.remotes {
        if rm.kind.as_deref() != Some("streamable-http") {
            continue;
        }
        let Some(url) = rm.url.as_deref() else {
            continue;
        };
        if !url.starts_with("https://") || url.contains('{') {
            continue;
        }
        for candidate in [Some(url)] {
            push_addr(&mut addrs, candidate);
        }
        let mut headers = serde_json::Map::new();
        for h in &rm.headers {
            let Some(name) = h.name.as_deref().filter(|n| !n.is_empty()) else {
                continue;
            };
            let value = h.value.as_deref().unwrap_or("");
            if !value.is_empty() && !has_placeholder(value) {
                headers.insert(name.to_string(), Value::String(value.to_string()));
            } else if h.is_required || h.is_secret {
                let format = if value.is_empty() {
                    "{}".to_string()
                } else {
                    // "Bearer {smithery_api_key}" keeps its words, loses the hole.
                    replace_placeholder(value)
                };
                service.inputs.push(MarketInput {
                    key: name.to_string(),
                    r#where: "header".into(),
                    label: name.to_string(),
                    description: h.description.clone().unwrap_or_default(),
                    placeholder: String::new(),
                    secret: h.is_secret,
                    required: h.is_required,
                    format,
                });
            }
        }
        let mut config = serde_json::Map::new();
        config.insert("url".into(), Value::String(url.to_string()));
        config.insert("transport".into(), Value::String("streamable-http".into()));
        if !headers.is_empty() {
            config.insert("headers".into(), Value::Object(headers));
        }
        service.config = Value::Object(config);
        service.transport = "streamable-http".into();
        return Some(service);
    }

    // Else the package it's published as.
    for want in ["npm", "pypi", "oci"] {
        for p in &r.packages {
            if p.registry_type.as_deref() != Some(want) {
                continue;
            }
            let Some(identifier) = p.identifier.as_deref().filter(|i| !i.is_empty()) else {
                continue;
            };
            let transport_kind = p.transport.as_ref().and_then(|t| t.kind.as_deref());
            if transport_kind.is_some_and(|k| k != "stdio") {
                continue;
            }
            let (command, base_args): (&str, Vec<String>) = match want {
                "npm" => ("npx", vec!["-y".into(), identifier.to_string()]),
                "pypi" => ("uvx", vec![identifier.to_string()]),
                _ => ("docker", vec!["run".into(), "-i".into(), "--rm".into()]),
            };
            let mut args = base_args;
            let mut config = serde_json::Map::new();
            for e in &p.environment_variables {
                if !e.is_required && !e.is_secret {
                    continue;
                }
                let Some(name) = e.name.as_deref().filter(|n| !n.is_empty()) else {
                    continue;
                };
                service.inputs.push(MarketInput {
                    key: name.to_string(),
                    r#where: "env".into(),
                    label: name.to_string(),
                    description: e.description.clone().unwrap_or_default(),
                    placeholder: String::new(),
                    secret: e.is_secret,
                    required: e.is_required,
                    format: String::new(),
                });
                if want == "oci" {
                    args.push("-e".into());
                    args.push(name.to_string());
                }
            }
            if want == "oci" {
                args.push(identifier.to_string());
            }
            // Package arguments: filled values join the command; required
            // positional ones become inputs; a required named one the market
            // can't fill drops the entry.
            for (i, a) in p.package_arguments.iter().enumerate() {
                let mut value = a
                    .value
                    .clone()
                    .or_else(|| a.default.clone())
                    .unwrap_or_default();
                if has_placeholder(&value) {
                    value = String::new();
                }
                let named = a.kind.as_deref() == Some("named");
                let name = a.name.clone().unwrap_or_default();
                if !value.is_empty() && named {
                    args.push(name);
                    args.push(value);
                } else if !value.is_empty() {
                    args.push(value);
                } else if a.is_required && !named {
                    let label = a
                        .value_hint
                        .clone()
                        .filter(|h| !h.is_empty())
                        .unwrap_or_else(|| format!("Argument {}", i + 1));
                    service.inputs.push(MarketInput {
                        key: format!("arg{i}"),
                        r#where: "arg".into(),
                        label,
                        description: a.description.clone().unwrap_or_default(),
                        placeholder: String::new(),
                        secret: false,
                        required: true,
                        format: String::new(),
                    });
                } else if a.is_required {
                    return None;
                }
            }
            config.insert("command".into(), Value::String(command.to_string()));
            config.insert(
                "args".into(),
                Value::Array(args.into_iter().map(Value::String).collect()),
            );
            config.insert("transport".into(), Value::String("stdio".into()));
            service.config = Value::Object(config);
            service.transport = "stdio".into();
            service.sign_in = false;
            return Some(service);
        }
    }
    None
}

fn push_addr(addrs: &mut Vec<String>, candidate: Option<&str>) {
    let Some(candidate) = candidate else {
        return;
    };
    let normalized = norm_addr(candidate);
    if !normalized.is_empty() && !addrs.contains(&normalized) {
        addrs.push(normalized);
    }
}

/// Where a registry entry lives: its name, page, repository, endpoints and
/// package identifiers, normalised, for the address search to score against.
fn registry_addrs(r: &RegistryServer) -> Vec<String> {
    let mut addrs = Vec::new();
    push_addr(&mut addrs, Some(&r.name));
    push_addr(&mut addrs, r.website_url.as_deref());
    push_addr(
        &mut addrs,
        r.repository.as_ref().and_then(|repo| repo.url.as_deref()),
    );
    for rm in &r.remotes {
        push_addr(&mut addrs, rm.url.as_deref());
    }
    for p in &r.packages {
        push_addr(&mut addrs, p.identifier.as_deref());
    }
    addrs
}

/// `"Bearer {smithery_api_key}"` becomes `"Bearer {}"`: the template the
/// panel fills the user's value into.
fn replace_placeholder(value: &str) -> String {
    let bytes = value.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b != b'{' {
            continue;
        }
        if let Some(close) = bytes[i + 1..].iter().position(|b| *b == b'}') {
            let mut out = String::from(&value[..i]);
            out.push_str("{}");
            out.push_str(&value[i + close + 2..]);
            return out;
        }
    }
    "{}".to_string()
}

/// What the registry lists for a search, best first, converted and deduped:
/// against the featured catalogue (whose entries the registry lists too) and
/// within itself.
async fn registry_search(q: &str) -> Result<Vec<MarketService>, String> {
    let trimmed = q.trim();
    let mut url = format!("{REGISTRY_URL}?limit=100&version=latest");
    if !trimmed.is_empty() {
        url.push_str("&search=");
        url.push_str(&urlencode(trimmed));
    }
    let response = REGISTRY_CLIENT
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| format!("couldn't reach the MCP Registry: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("the MCP Registry answered {}", response.status()));
    }
    let body: RegistryResponse = response
        .json()
        .await
        .map_err(|error| format!("the MCP Registry's answer didn't parse: {error}"))?;

    let addr = address(trimmed);
    let terms = addr.as_deref().map(addr_terms);
    let mut by_name: Vec<(MarketService, i32)> = Vec::new();
    let mut matched: Vec<(MarketService, i32)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for entry in body.servers {
        let status = entry
            ._meta
            .get("io.modelcontextprotocol.registry/official")
            .and_then(|meta| meta.status.clone());
        if status.is_some_and(|s| s != "active") {
            continue;
        }
        let r = &entry.server;
        if r.name.is_empty() {
            continue;
        }
        let Some(mut service) = from_registry(r) else {
            continue;
        };
        let key = service.server_key();
        if FEATURED_KEYS.contains(&key) || !seen.insert(key) {
            continue;
        }
        let query = trimmed.to_lowercase();
        let base = rank(r, &service, &query);
        match (&addr, &terms) {
            // An address search: what is at the address wins, scored by how
            // well the address matches; whatever the first derived name
            // brought in stands in when nothing matched, as a search for
            // that name would.
            (Some(a), Some(_terms)) => {
                let addrs = registry_addrs(r);
                let score = addr_score(a, &addrs);
                if score > 0 {
                    service.matched = true;
                    matched.push((service, (score * 1000) as i32 + base));
                } else {
                    by_name.push((service, base));
                }
            }
            _ => by_name.push((service, base)),
        }
    }
    matched.sort_by(|a, b| b.1.cmp(&a.1));
    by_name.sort_by(|a, b| b.1.cmp(&a.1));
    let out: Vec<MarketService> = matched
        .into_iter()
        .chain(by_name)
        .map(|(service, _)| service)
        .collect();
    Ok(out)
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The registry's answer for a query, from the cache when it is fresh.
async fn registry_cached(q: &str) -> Result<std::sync::Arc<Vec<MarketService>>, String> {
    let key = q.trim().to_lowercase();
    if let Ok(cache) = REGISTRY_CACHE.lock() {
        if let Some(entry) = cache.get(&key) {
            if entry.at.elapsed() < CACHE_TTL {
                return Ok(Arc::clone(&entry.items));
            }
        }
    }
    let items = registry_search(q).await?;
    let items = std::sync::Arc::new(items);
    if let Ok(mut cache) = REGISTRY_CACHE.lock() {
        if cache.len() >= CACHE_CAP {
            cache.clear();
        }
        cache.insert(
            key,
            CacheEntry {
                at: Instant::now(),
                items: Arc::clone(&items),
            },
        );
    }
    Ok(items)
}

// ---- handlers ---------------------------------------------------------------

/// GET /plugins/market/services: the curated catalogue, compiled in.
pub(super) async fn services_market_featured() -> ApiResult {
    Ok(success(
        "Featured services fetched successfully",
        json!({ "items": &*FEATURED }),
    ))
}

/// GET /plugins/market/services/registry?q=&offset=&limit=: one page of the
/// registry's results, best first. A registry failure is reported in
/// `failed` and never takes the featured catalogue down with it (#1589's
/// semantics): the page is empty, the next try fetches again.
pub(super) async fn services_market_registry(
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult {
    let q = params.get("q").map(String::as_str).unwrap_or("");
    let offset: usize = params
        .get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let limit: usize = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(20)
        .min(50);
    match registry_cached(q).await {
        Ok(items) => {
            let total = items.len();
            let page: Vec<&MarketService> = items.iter().skip(offset).take(limit).collect();
            Ok(success(
                "Registry services fetched successfully",
                json!({ "items": page, "offset": offset, "total": total }),
            ))
        }
        Err(error) => Ok(success(
            "Registry services fetched with a failure",
            json!({ "items": [], "offset": offset, "total": 0, "failed": error }),
        )),
    }
}

// ---- tests ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn featured_configs_are_installable() {
        for service in FEATURED.iter() {
            let has_url = service.config.get("url").is_some();
            let has_command = service.config.get("command").is_some();
            assert!(
                has_url ^ has_command,
                "{} has neither or both of url/command",
                service.id
            );
            let transport = service
                .config
                .get("transport")
                .and_then(Value::as_str)
                .unwrap();
            assert!(
                matches!(transport, "streamable-http" | "stdio"),
                "{}",
                service.id
            );
            assert!(!service.title.is_empty(), "{}", service.id);
            for input in &service.inputs {
                assert!(
                    matches!(input.r#where.as_str(), "env" | "header" | "arg"),
                    "{} input {}",
                    service.id,
                    input.key
                );
            }
        }
    }

    #[test]
    fn featured_keys_are_distinct() {
        let keys: HashSet<String> = FEATURED.iter().map(|s| s.server_key()).collect();
        assert_eq!(keys.len(), FEATURED.len());
    }

    #[test]
    fn norm_addr_normalises_every_shape() {
        assert_eq!(norm_addr("https://GitHub.com/o/r.git/"), "github.com/o/r");
        assert_eq!(norm_addr("git@github.com:o/r.git"), "github.com/o/r");
        assert_eq!(norm_addr("https://user@ghcr.io/o/img:1.2"), "ghcr.io/o/img");
        assert_eq!(
            norm_addr("https://www.npmjs.com/package/@scope/pkg"),
            "@scope/pkg"
        );
        assert_eq!(
            norm_addr("https://pypi.org/project/mcp-server-fetch"),
            "mcp-server-fetch"
        );
        assert_eq!(norm_addr("mcp.context7.com/mcp"), "mcp.context7.com/mcp");
    }

    #[test]
    fn address_reads_addresses_and_not_words() {
        assert_eq!(address("github.com/o/r"), Some("github.com/o/r".into()));
        assert_eq!(address("o/r"), Some("o/r".into()));
        assert_eq!(address("@scope/pkg"), Some("@scope/pkg".into()));
        assert_eq!(
            address("https://mcp.notion.com/mcp"),
            Some("mcp.notion.com/mcp".into())
        );
        assert_eq!(address("context7"), None);
        assert_eq!(address("two words"), None);
    }

    #[test]
    fn addr_terms_picks_two_names() {
        assert_eq!(
            addr_terms("github.com/crystaldba/postgres-mcp"),
            vec!["postgres-mcp", "crystaldba"]
        );
        // magpie skips names shorter than two characters.
        assert_eq!(addr_terms("github.com/o/r"), Vec::<String>::new());
        assert_eq!(addr_terms("@scope/pkg"), vec!["pkg", "scope"]);
        assert_eq!(addr_terms("mcp.context7.com/mcp"), vec!["context7", "mcp"]);
        assert_eq!(
            addr_terms("docs.mcp.cloudflare.com/mcp"),
            vec!["cloudflare", "mcp"]
        );
    }

    #[test]
    fn addr_scores_by_depth() {
        let addrs = vec!["github.com/o/repo".to_string()];
        assert_eq!(addr_score("github.com/o/repo", &addrs), 3);
        assert_eq!(addr_score("o/repo", &addrs), 3); // host-less query matches any host
                                                     // A repository (or owner) scores 2 for a server inside it.
        assert_eq!(addr_score("github.com/o/repo/sub", &addrs), 1);
        assert_eq!(addr_score("github.com/o", &addrs), 2);
        assert_eq!(addr_score("github.com/other/repo", &addrs), 0);
    }

    #[test]
    fn short_name_strips_the_mcp_noise() {
        assert_eq!(short_name("io.github.upstash/context7-mcp"), "context7");
        assert_eq!(
            short_name("io.github.modelcontextprotocol/server-filesystem"),
            "filesystem"
        );
        assert_eq!(short_name("com.example/mcp-server-git"), "git");
        assert_eq!(short_name("io.github.x/__"), "server");
    }

    #[test]
    fn server_keys_identify_the_same_service() {
        let remote = json!({"url": "https://mcp.context7.com/mcp", "transport": "streamable-http"});
        let local = json!({"command": "npx", "args": ["-y", "@playwright/mcp@latest"], "transport": "stdio"});
        assert_eq!(server_key(&remote), "https://mcp.context7.com/mcp");
        assert_eq!(server_key(&local), "npx @playwright/mcp");
    }

    #[test]
    fn replace_placeholder_keeps_the_words() {
        assert_eq!(
            replace_placeholder("Bearer {smithery_api_key}"),
            "Bearer {}"
        );
        assert_eq!(replace_placeholder("{}"), "{}");
        assert_eq!(replace_placeholder("no hole"), "{}");
    }

    #[test]
    fn registry_answer_converts_and_ranks() {
        let body: RegistryResponse = serde_json::from_str(
            r#"{"servers":[
              {"server":{"name":"io.github.upstash/context7-mcp","title":"Context7",
                "description":"docs","websiteUrl":"https://context7.com",
                "repository":{"url":"https://github.com/upstash/context7"},
                "remotes":[{"type":"streamable-http","url":"https://mcp.context7.com/mcp"}]},
               "_meta":{"io.modelcontextprotocol.registry/official":{"status":"active"}}},
              {"server":{"name":"io.github.gone/dead-mcp",
                "remotes":[{"type":"streamable-http","url":"https://x.example/mcp"}]},
               "_meta":{"io.modelcontextprotocol.registry/official":{"status":"deleted"}}}
            ]}"#,
        )
        .expect("registry fixture parses");
        assert_eq!(body.servers.len(), 2);
        let service = from_registry(&body.servers[0].server).expect("converts");
        assert_eq!(service.name, "context7");
        assert_eq!(service.transport, "streamable-http");
        assert_eq!(
            service.config.get("url").and_then(Value::as_str),
            Some("https://mcp.context7.com/mcp")
        );
        assert_eq!(service.publisher, "upstash");
        // Its key collides with the featured context7: it would be dropped.
        assert!(FEATURED_KEYS.contains(&service.server_key()));
    }

    #[test]
    fn sse_only_entries_are_skipped() {
        let r: RegistryServer = serde_json::from_str(
            r#"{"name":"io.github.x/old-sse",
                "remotes":[{"type":"sse","url":"https://x.example/sse"}]}"#,
        )
        .expect("fixture parses");
        assert!(from_registry(&r).is_none());
    }

    #[test]
    fn npm_package_becomes_a_command() {
        let r: RegistryServer = serde_json::from_str(
            r#"{"name":"io.github.y/tool-mcp",
                "packages":[{"registryType":"npm","identifier":"@y/tool",
                  "transport":{"type":"stdio"},
                  "environmentVariables":[{"name":"Y_KEY","isRequired":true,"isSecret":true,
                    "description":"the key"}]}]}"#,
        )
        .expect("fixture parses");
        let service = from_registry(&r).expect("converts");
        assert_eq!(service.transport, "stdio");
        assert_eq!(
            service.config.get("command").and_then(Value::as_str),
            Some("npx")
        );
        assert_eq!(
            service
                .config
                .get("args")
                .and_then(Value::as_array)
                .map(|a| a.len()),
            Some(2)
        );
        assert_eq!(service.inputs.len(), 1);
        assert_eq!(service.inputs[0].key, "Y_KEY");
        assert!(service.inputs[0].required && service.inputs[0].secret);
    }
}
