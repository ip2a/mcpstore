use maud::{html, Markup, PreEscaped, DOCTYPE};

pub(super) fn layout(title: &str, content: Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                link rel="stylesheet" href="/assets/style.css";
            }
            body {
                div.app-shell {
                    nav.topbar aria-label="Main navigation" {
                        a.brand href="/" aria-label="mcpstore home" {
                            span.brand-mark aria-hidden="true" { "m" }
                            span.brand-text { "mcpstore" }
                        }
                        div.top-actions {
                            a.button.button-ghost href="/" { "Services" }
                            a.button.button-primary href="/add" { "Add" }
                            span.version { (format!("v{}", env!("CARGO_PKG_VERSION"))) }
                        }
                    }
                    main {
                        (content)
                    }
                }
                div #modal-container {}
                div.loading-layer aria-live="polite" aria-label="Loading" {
                    div.loading-card {
                        span.loading-spinner aria-hidden="true" {}
                        span { "Loading" }
                    }
                }
                (client_script())
            }
        }
    }
}

fn client_script() -> Markup {
    html! {
        script {
            (PreEscaped(r#"
function setLoading(active) {
    document.body.classList.toggle('page-loading', Boolean(active));
}

function mountModal(html) {
    const container = document.getElementById('modal-container');
    container.innerHTML = html;
    const dialog = container.querySelector('dialog');
    if (!dialog) return;
    dialog.removeAttribute('open');
    dialog.addEventListener('click', function(event) {
        if (event.target === dialog) closeModal();
    });
    dialog.showModal();
}

function closeModal() {
    const container = document.getElementById('modal-container');
    const dialog = container.querySelector('dialog');
    if (dialog) dialog.close();
    container.innerHTML = '';
}

function mountError(message) {
    mountModal('<dialog><article><header><h3>Error</h3><button type="button" onclick="closeModal()">Close</button></header><p></p><footer><button type="button" onclick="closeModal()">Close</button></footer></article></dialog>');
    const p = document.querySelector('#modal-container p');
    if (p) p.textContent = message;
}

window.addEventListener('pageshow', function() {
    setLoading(false);
});

document.addEventListener('click', function(event) {
    const trigger = event.target.closest('[data-modal]');
    if (trigger) {
        event.preventDefault();
        setLoading(true);
        fetch(trigger.getAttribute('data-modal'))
            .then(r => { if (!r.ok) throw new Error('Load failed'); return r.text(); })
            .then(html => { setLoading(false); mountModal(html); })
            .catch(e => { setLoading(false); mountError(e.message); });
        return;
    }

    const copyBtn = event.target.closest('[data-copy]');
    if (copyBtn) {
        event.preventDefault();
        const orig = copyBtn.textContent;
        if (!navigator.clipboard) {
            copyBtn.textContent = 'Copy failed';
            setTimeout(() => copyBtn.textContent = orig, 1200);
            return;
        }
        navigator.clipboard.writeText(copyBtn.dataset.copy).then(() => {
            copyBtn.textContent = 'Copied';
            setTimeout(() => copyBtn.textContent = orig, 1200);
        }).catch(() => {
            copyBtn.textContent = 'Copy failed';
            setTimeout(() => copyBtn.textContent = orig, 1200);
        });
        return;
    }

    const link = event.target.closest('a[href]');
    if (!link || link.target || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const href = link.getAttribute('href');
    if (!href || href.startsWith('#') || href.startsWith('javascript:')) return;
    const url = new URL(href, window.location.href);
    if (url.origin !== window.location.origin) return;
    setLoading(true);
});

document.addEventListener('submit', function(event) {
    const form = event.target;
    if (form.action && form.action.includes && form.action.includes('/modal/')) {
        event.preventDefault();
        setLoading(true);
        const url = new URL(form.action, window.location.href);
        const params = new URLSearchParams(new FormData(form));
        url.search = params.toString();
        fetch(url.toString())
            .then(r => { if (!r.ok) throw new Error('Load failed'); return r.text(); })
            .then(html => { setLoading(false); mountModal(html); })
            .catch(e => { setLoading(false); mountError(e.message); });
        return;
    }
    setLoading(true);
});
"#))
        }
    }
}
