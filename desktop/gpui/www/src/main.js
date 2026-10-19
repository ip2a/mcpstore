const embedded = window.parent !== window;

// `gpui_web` reads keyboard/IME input through a 1x1 transparent <input> it
// appends to the body (same handling as the upstream gpui-kit example):
// on touch-only devices mark it readonly so the on-screen keyboard stays down.
const touchOnly =
  window.matchMedia?.('(hover: none) and (pointer: coarse)').matches ?? false;

function keepKeyboardClosed(node) {
  if (node.tagName !== 'INPUT') return;
  if (touchOnly) {
    node.readOnly = true;
    node.setAttribute('inputmode', 'none');
  }
  if ((touchOnly || embedded) && document.activeElement === node) {
    node.blur();
  }
}

function watchPlatformInput() {
  document.querySelectorAll('body > input').forEach(keepKeyboardClosed);
  new MutationObserver((records) => {
    for (const record of records) {
      record.addedNodes.forEach(keepKeyboardClosed);
    }
  }).observe(document.body, { childList: true });
}

async function init() {
  const loading = document.getElementById('loading');
  watchPlatformInput();
  try {
    const wasm = await import('./wasm/mcpstore_desktop.js');
    await wasm.default();
    await wasm.start();
    loading?.remove();
  } catch (error) {
    console.error('Failed to initialize mcpstore panel:', error);
    if (loading) loading.textContent = `Failed to load panel: ${error?.message || error}`;
  }
}
init();
