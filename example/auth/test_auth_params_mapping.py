"""
Example test: verify auth params (token/api_key/headers) are standardized into headers only.
These are example-level tests (not pytest) and can be run as a script.
They call the internal helper to avoid real network connections.
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


def test_apply_auth_to_config_single_service():
    store = MCPStore.setup_store()
    ctx = store.for_store()

    cfg = {
        "name": "svc",
        "url": "https://example.com/mcp"
    }
    result = ctx._apply_auth_to_config(cfg, auth=None, token="TKN", api_key=None, headers=None)
    assert_equal(result["headers"]["Authorization"], "Bearer TKN", "Bearer header missing")
    assert "token" not in result and "api_key" not in result and "auth" not in result

    result2 = ctx._apply_auth_to_config(cfg, auth=None, token=None, api_key="K123", headers=None)
    assert_equal(result2["headers"]["X-API-Key"], "K123", "X-API-Key header missing")

    # explicit headers win
    result3 = ctx._apply_auth_to_config(cfg, auth=None, token="T1", api_key="K1", headers={"Authorization": "Bearer OVERRIDE"})
    assert_equal(result3["headers"]["Authorization"], "Bearer OVERRIDE")


def test_apply_auth_to_config_mcpServers():
    store = MCPStore.setup_store()
    ctx = store.for_store()

    cfg = {
        "mcpServers": {
            "svc": {"url": "https://example.com/mcp"}
        }
    }
    result = ctx._apply_auth_to_config(cfg, auth=None, token="ABC", api_key="KEY", headers={"X-Tenant": "acme"})
    headers = result["mcpServers"]["svc"]["headers"]
    assert_equal(headers["Authorization"], "Bearer ABC")
    assert_equal(headers["X-API-Key"], "KEY")
    assert_equal(headers["X-Tenant"], "acme")


if __name__ == "__main__":
    test_apply_auth_to_config_single_service()
    test_apply_auth_to_config_mcpServers()
    print("OK - auth params mapping works")

