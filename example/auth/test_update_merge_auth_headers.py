"""
Example test: verify update_service uses merge semantics and updates only provided keys.
"""
import os, sys
CURR_DIR = os.path.dirname(__file__)
REPO_ROOT = os.path.abspath(os.path.join(CURR_DIR, "..", ".."))
SRC_DIR = os.path.join(REPO_ROOT, "src")
if os.path.isdir(SRC_DIR) and SRC_DIR not in sys.path:
    sys.path.insert(0, SRC_DIR)
from mcpstore import MCPStore


def assert_equal(a, b, msg=""):
    if a != b:
        raise AssertionError(msg or f"Expected {b!r}, got {a!r}")


def test_update_merge_headers():
    store = MCPStore.setup_store()
    ctx = store.for_store()

    # start with a config in registry and on-disk cache (avoid disk by mcp_json=None)
    cfg = {"name": "svc", "url": "https://example.com/mcp", "headers": {"X-Env": "dev"}}
    ctx.add_service(cfg, token="T0")

    # update only token; should merge into headers, keep url and X-Env
    ctx.update_service("svc", token="T1")

    services = ctx.list_services()
    svc = next(s for s in services if s["name"] == "svc")
    headers = svc.get("headers", {})

    assert_equal(headers.get("Authorization"), "Bearer T1")
    assert_equal(headers.get("X-Env"), "dev")
    assert_equal(svc.get("url"), "https://example.com/mcp")


if __name__ == "__main__":
    test_update_merge_headers()
    print("OK - update_service merge semantics verified")

