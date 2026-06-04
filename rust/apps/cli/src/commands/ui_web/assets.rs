use axum::{http::header, response::IntoResponse};

pub(super) async fn serve_css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        CUSTOM_CSS,
    )
}

pub(super) async fn serve_favicon() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "image/svg+xml")], FAVICON_SVG)
}

const FAVICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" rx="0" fill="#fff" stroke="#000" stroke-width="2"/><rect x="12" y="12" width="40" height="40" rx="0" fill="#000"/><path d="M22 42V22h5l5 8 5-8h5v20h-5V30l-5 8-5-8v12z" fill="#fff"/></svg>"##;

const CUSTOM_CSS: &str = r#"
:root {
    --bg: #fff;
    --surface: #fff;
    --surface-muted: #f5f5f5;
    --ink: #000;
    --muted: #666;
    --faint: #999;
    --line: #000;
    --line-sub: #ccc;
    --radius: 0px;
    --radius-sm: 0px;
    color-scheme: light;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, "Courier New", monospace;
    font-size: 14px;
    letter-spacing: 0;
}

* { box-sizing: border-box; }
html, body { margin: 0; min-height: 100%; background: var(--bg); color: var(--ink); }
body { line-height: 1.5; }
a { color: inherit; text-decoration: none; }
h1, h2, h3, p { margin: 0; }
h1 { font-size: 28px; line-height: 1.1; font-weight: 700; text-transform: uppercase; }
h2 { font-size: 16px; line-height: 1.2; font-weight: 700; text-transform: uppercase; }
h3 { font-size: 14px; line-height: 1.25; font-weight: 700; text-transform: uppercase; }
code, pre, .version, .eyebrow, .control-label, .meta-pill, .transport-pill, .status-badge {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, "Courier New", monospace;
}
code { font-size: 12px; overflow-wrap: anywhere; }

button, input, select, textarea, .button {
    font: inherit;
    color: var(--ink);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
}
button, .button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 32px;
    padding: 0 12px;
    cursor: pointer;
    white-space: nowrap;
    transition: background 100ms ease, color 100ms ease;
}
button:hover, .button:hover {
    background: #000;
    color: #fff;
}
button:active, .button:active { background: #333; }
button:focus-visible, .button:focus-visible, input:focus-visible, select:focus-visible, textarea:focus-visible {
    outline: 2px solid #000;
    outline-offset: 2px;
}
.button.invert {
    background: #000;
    border-color: #000;
    color: #fff;
}
.button.invert:hover {
    background: #fff;
    color: #000;
}
.button.warn {
    color: #000;
    border-color: #000;
}
.button.warn:hover {
    background: #000;
    color: #fff;
}

.app-shell {
    width: min(1180px, calc(100vw - 32px));
    margin: 0 auto;
    padding-bottom: 48px;
}
.topbar {
    height: 64px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 24px;
}
.brand, .top-actions {
    display: flex;
    align-items: center;
    gap: 10px;
}
.brand { font-weight: 700; text-transform: uppercase; }
.brand-text { font-size: 14px; }
.settings-entry { min-width: 72px; }
.version {
    color: var(--muted);
    font-size: 11px;
    padding-left: 4px;
}
.github-link, .icon-button {
    width: 34px;
    height: 34px;
    display: inline-grid;
    place-items: center;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    transition: background 100ms ease, color 100ms ease;
    padding: 0;
}
.github-link:hover, .icon-button:hover { background: var(--ink); color: var(--surface); }
.github-link:focus-visible, .icon-button:focus-visible { outline: 2px solid var(--ink); outline-offset: 2px; }
.github-link svg, .icon-button svg { width: 16px; height: 16px; }
.lang-switch {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
}
.lang-switch select { height: 34px; padding: 0 8px; }

.page-heading {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 18px;
    align-items: start;
    margin-bottom: 20px;
}
.heading-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
}
.eyebrow {
    color: var(--muted);
    font-size: 11px;
    text-transform: uppercase;
    margin-bottom: 6px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

.ascii-banner {
    display: grid;
    place-items: center;
    margin: 6px 0 24px;
    padding: 20px 0;
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
    overflow: hidden;
}
.ascii-banner pre {
    margin: 0;
    border: 0;
    padding: 0;
    overflow: visible;
    max-width: none;
    text-align: left;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: clamp(6px, 0.88vw, 12px);
    line-height: 1;
    font-weight: 900;
    letter-spacing: 0.02em;
    white-space: pre;
}

.home-panel {
    display: grid;
    grid-template-columns: minmax(260px, 0.72fr) minmax(420px, 1.28fr);
    gap: 18px;
    align-items: stretch;
    margin: 18px 0 22px;
    padding: 18px 0;
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line);
}
.home-hero {
    display: grid;
    grid-template-rows: auto 1fr auto;
    gap: 12px;
    min-height: 118px;
    padding-right: 18px;
    border-right: 1px solid var(--line);
}
.home-stats {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
}
.readonly-pill {
    display: inline-flex;
    align-items: center;
    min-height: 28px;
    padding: 3px 8px;
    border: 1px solid var(--line);
    background: var(--surface);
    color: var(--ink);
    text-transform: uppercase;
}
.home-actions {
    display: grid;
    gap: 12px;
    align-content: start;
}
.query-block {
    display: grid;
    gap: 8px;
}
.control-title {
    color: var(--ink);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
}
.home-search-form {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
}
.home-search-form input {
    flex: 1 1 480px;
    min-height: 42px;
    padding: 8px 11px;
    font-size: 15px;
}
.home-search-form input::placeholder { color: var(--muted); }
.control-row {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    flex-wrap: wrap;
}
.scope-row {
    align-items: center;
}
.action-row {
    padding-top: 2px;
    justify-content: flex-end;
}
.filter-group {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
}
.home-filter-group {
    align-items: flex-start;
}
.filter-label {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 11px;
    text-transform: uppercase;
    color: var(--muted);
    min-width: 78px;
    padding-top: 6px;
}
.data-source .meta-pill {
    min-height: 28px;
    padding: 2px 8px;
}

.filter-modal-form {
    gap: 16px;
}
.filter-modal-section {
    display: grid;
    gap: 8px;
    padding: 12px 0;
    border-top: 1px solid var(--line);
}
.filter-modal-section:first-child {
    border-top: 0;
    padding-top: 0;
}
.filter-modal-title {
    color: var(--muted);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
}
.filter-tag-grid {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}
.filter-tag {
    display: inline-flex;
    cursor: pointer;
}
.filter-tag input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
}
.filter-tag span {
    display: inline-flex;
    align-items: center;
    min-height: 30px;
    padding: 4px 10px;
    border: 1px solid var(--line);
    background: var(--surface);
    color: var(--ink);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 11px;
    text-transform: uppercase;
}
.filter-tag input:checked + span {
    background: var(--ink);
    color: var(--surface);
}
.filter-tag:focus-within span {
    outline: 2px solid #000;
    outline-offset: 2px;
}

.dashboard-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 12px;
    margin-bottom: 14px;
}
.metric-card {
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 14px;
    display: grid;
    gap: 6px;
}
.metric-value {
    font-size: 24px;
    line-height: 1;
    font-weight: 700;
}
.metric-label {
    color: var(--muted);
    font-size: 11px;
    text-transform: uppercase;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

.control-strip {
    display: grid;
    gap: 10px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 12px;
    margin-bottom: 22px;
}
.control-group {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    min-width: 0;
}
.control-label {
    color: var(--muted);
    font-size: 11px;
    min-width: 54px;
    text-transform: uppercase;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
.control-group-wide { align-items: flex-start; }
.filter-chips {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
}
.chip, .meta-pill, .transport-pill, .status-badge {
    display: inline-flex;
    align-items: center;
    min-height: 26px;
    padding: 3px 9px;
    border: 1px solid var(--line);
    border-radius: 0;
    background: var(--surface);
    color: var(--ink);
    font-size: 11px;
    overflow-wrap: anywhere;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    text-transform: uppercase;
}
.chip.is-active {
    background: var(--ink);
    border-color: var(--ink);
    color: #fff;
}
.status-badge.ok {
    background: transparent;
    border-color: transparent;
    color: #000;
    font-weight: 700;
}
.status-badge.off {
    background: transparent;
    border-color: transparent;
    color: var(--muted);
}
.status-badge.wait {
    background: transparent;
    border-color: transparent;
    color: #000;
}
.status-badge.err {
    background: transparent;
    border-color: transparent;
    color: #000;
    font-weight: 700;
}

.list-section, .content-section {
    display: grid;
    gap: 12px;
    margin-top: 22px;
}
.section-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
}
.section-count {
    color: var(--muted);
    font-size: 12px;
}
.service-table {
    overflow: hidden;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius);
}
.service-table-head, .service-row {
    display: grid;
    grid-template-columns: minmax(180px, 260px) minmax(92px, 0.58fr) minmax(88px, 0.45fr) 64px minmax(220px, auto);
    gap: 16px;
    align-items: center;
}
.service-table-head {
    min-height: 36px;
    padding: 0 14px;
    background: var(--surface-muted);
    color: var(--muted);
    font-size: 11px;
    border-bottom: 1px solid var(--line);
    text-transform: uppercase;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
.sort-link {
    color: var(--muted);
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
}
.sort-link:hover {
    color: var(--ink);
}
.sort-indicator {
    font-size: 10px;
    color: var(--ink);
}
.sort-indicator.placeholder {
    color: var(--line-sub);
}
.service-row {
    min-height: 64px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--line);
}
.service-row:last-child { border-bottom: 0; }
.service-row:hover { background: #f5f5f5; }

.service-list-open {
    display: grid;
    border-top: 1px solid var(--line);
}
.service-list-open .service-table-head {
    background: transparent;
    color: var(--muted);
}
.service-list-open .service-row:last-child { border-bottom: 1px solid var(--line); }
.service-list-open .service-row:hover {
    background: var(--ink);
    color: var(--surface);
}
.service-list-open .service-row:hover a,
.service-list-open .service-row:hover .service-name,
.service-list-open .service-row:hover .service-desc,
.service-list-open .service-row:hover .service-group,
.service-list-open .service-row:hover .status-badge,
.service-list-open .service-row:hover .service-tools {
    color: var(--surface);
}
.service-list-open .service-row:hover .row-actions .button {
    border-color: var(--surface);
    color: var(--surface);
}
.service-list-open .service-row:hover .row-actions .button:hover {
    background: var(--surface);
    color: var(--ink);
}
.service-row > * { min-width: 0; }
.service-main {
    display: grid;
    gap: 3px;
    min-width: 0;
    max-width: 260px;
}
.service-name {
    font-weight: 700;
    overflow-wrap: anywhere;
    word-break: break-word;
    white-space: normal;
    line-height: 1.25;
}
.service-name:hover { text-decoration: underline; }
.service-desc {
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.service-group {
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.service-tools {
    color: var(--ink);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
    font-weight: 700;
}
.row-actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    flex-wrap: wrap;
}
.row-actions .button {
    min-height: 28px;
    padding: 0 9px;
    font-size: 11px;
}

.empty-state {
    display: grid;
    justify-items: center;
    gap: 10px;
    text-align: center;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 34px 24px;
}
.empty-state p {
    color: var(--muted);
    max-width: 520px;
}
.empty-state.compact {
    padding: 22px;
}

.service-meta {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 12px;
}
.detail-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(210px, 1fr));
    gap: 12px;
    margin-bottom: 22px;
}
.detail-item, .detail-block {
    display: grid;
    gap: 6px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 12px;
    min-width: 0;
}
.detail-label {
    color: var(--muted);
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    text-transform: uppercase;
}

.tool-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 12px;
}
.tool-card {
    display: flex;
    flex-direction: column;
    min-width: 0;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
}
.tool-card-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 12px;
    border-bottom: 1px solid var(--line);
}
.tool-title {
    display: grid;
    gap: 3px;
    min-width: 0;
}
.tool-name {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-weight: 700;
    overflow-wrap: anywhere;
    word-break: break-all;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
}
.tool-meta {
    color: var(--muted);
    font-size: 11px;
    text-transform: uppercase;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
.tool-card-actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    flex-wrap: wrap;
}
.tool-card-actions .button {
    min-height: 28px;
    padding: 0 9px;
    font-size: 11px;
}
.tool-description {
    color: var(--muted);
    font-size: 12px;
    line-height: 1.6;
    padding: 12px;
    border-bottom: 1px solid var(--line);
    overflow-wrap: anywhere;
    word-break: break-all;
}
.param-list {
    display: grid;
    gap: 8px;
    padding: 12px;
}
.param-item {
    display: grid;
    gap: 4px;
}
.param-main {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
}
.param-name {
    background: #000;
    color: #fff;
    padding: 2px 7px;
    border-radius: 0;
}
.param-type, .param-required {
    color: var(--muted);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 10px;
    text-transform: uppercase;
}
.param-required { color: #000; font-weight: 700; }
.param-desc {
    color: var(--muted);
    font-size: 11px;
    overflow-wrap: anywhere;
}
.param-empty {
    color: var(--muted);
    font-size: 12px;
    padding: 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}

.code-block, pre {
    overflow: auto;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 12px;
    margin: 0;
    background: var(--surface);
    color: var(--ink);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
    line-height: 1.45;
}

.add-form {
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 16px;
}
.form-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
}
.field {
    display: grid;
    gap: 6px;
    min-width: 0;
}
.field-wide { grid-column: 1 / -1; }
.field label {
    color: var(--muted);
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    text-transform: uppercase;
}
.field input, .field select, .field textarea {
    width: 100%;
    min-height: 36px;
    padding: 8px 10px;
}
.field textarea {
    min-height: 116px;
    resize: vertical;
    line-height: 1.45;
}
.field input::placeholder, .field textarea::placeholder {
    color: var(--faint);
}
.form-footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
}

.page-loading main, .page-loading .topbar {
    opacity: 0.48;
    pointer-events: none;
}
.loading-layer {
    position: fixed;
    inset: 0;
    display: none;
    place-items: center;
    z-index: 20;
    background: rgba(255, 255, 255, 0.9);
}
.page-loading .loading-layer { display: grid; }
.loading-card {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 11px;
    text-transform: uppercase;
}
.loading-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid #000;
    border-right-color: transparent;
    border-radius: 0;
    animation: spin 600ms linear infinite;
}
@keyframes spin { to { transform: rotate(360deg); } }

dialog {
    width: min(640px, calc(100vw - 32px));
    border: 2px solid var(--line);
    border-radius: var(--radius);
    padding: 0;
    color: var(--ink);
    background: var(--surface);
}
dialog::backdrop { background: rgba(255, 255, 255, 0.85); }
dialog article { padding: 18px; }
.modal-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 14px;
    margin-bottom: 14px;
}
dialog footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
}
.modal-form {
    display: grid;
    gap: 12px;
}
.hint {
    color: var(--muted);
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
.modal-message {
    color: var(--muted);
    overflow-wrap: anywhere;
}
.modal-stack {
    display: grid;
    gap: 12px;
}
.result-block {
    max-height: 360px;
    white-space: pre-wrap;
}
.result-block.is-error {
    border-color: #000;
    color: #000;
    font-weight: 700;
}
.settings-list { display: grid; border-top: 1px solid var(--line); }
.settings-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(180px, auto);
    gap: 14px;
    align-items: center;
    min-height: 54px;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
}
.settings-copy { display: grid; gap: 2px; min-width: 0; }
.settings-copy strong { font-size: 14px; }
.settings-copy span {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
    overflow-wrap: anywhere;
    color: var(--muted);
}
.settings-row select,
.settings-row input[type="text"] {
    width: 100%;
    min-width: 180px;
    height: 34px;
    padding: 0 8px;
}
.settings-note {
    color: var(--muted);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 11px;
    text-transform: uppercase;
    text-align: right;
}

@media (max-width: 860px) {
    .app-shell { width: min(100vw - 20px, 1180px); }
    h1 { font-size: 24px; }
    .topbar, .page-heading {
        grid-template-columns: 1fr;
    }
    .topbar {
        align-items: flex-start;
        padding: 14px 0;
    }
    .top-actions, .heading-actions {
        justify-content: flex-start;
    }
    .home-panel { grid-template-columns: 1fr; gap: 14px; }
    .home-hero { min-height: auto; padding-right: 0; padding-bottom: 14px; border-right: 0; border-bottom: 1px solid var(--line); }
    .home-actions { grid-template-columns: 1fr; }
    .service-table-head { display: none; }
    .service-row {
        grid-template-columns: 1fr;
        gap: 8px;
        align-items: start;
    }
    .row-actions { justify-content: flex-start; }
    .form-grid { grid-template-columns: 1fr; }
    .tool-grid { grid-template-columns: 1fr; }
    .tool-card-header {
        display: grid;
    }
    .tool-card-actions {
        justify-content: flex-start;
    }
}

@media (max-width: 520px) {
    h1 { font-size: 20px; }
    .top-actions, .heading-actions, .form-footer, dialog footer {
        width: 100%;
    }
    .top-actions .button, .heading-actions .button, .heading-actions button, .form-footer .button, .form-footer button {
        flex: 1 1 auto;
    }
    .dashboard-grid {
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    dialog { width: calc(100vw - 20px); }
}
"#;
