'''
Author: whill ooooofish@126.com
Date: 2025-10-14 23:33:53
LastEditors: whill ooooofish@126.com
LastEditTime: 2025-10-15 00:01:44
FilePath: \mcpstore\example\auth\test_failure_reason_demo.py
Description: 这是默认设置,请设置`customMade`, 打开koroFileHeader查看配置 进行设置: https://github.com/OBKoro1/koro1FileHeader/wiki/%E9%85%8D%E7%BD%AE
'''
"""
Example demo: show how to read failure_reason from service metadata after a failed connection.
Note: This script does not force a real 401/403; it demonstrates the access pattern.
"""
import os, sys
CURR_DIR = os.path.dirname(__file__)
REPO_ROOT = os.path.abspath(os.path.join(CURR_DIR, "..", ".."))
SRC_DIR = os.path.join(REPO_ROOT, "src")
if os.path.isdir(SRC_DIR) and SRC_DIR not in sys.path:
    sys.path.insert(0, SRC_DIR)
from mcpstore import MCPStore


def main():
    store = MCPStore.setup_store()
    ctx = store.for_store()

    # Add a service with an obviously bad token to illustrate the flow.
    # If the target URL is unreachable, failure_reason may be None or network_error.
    ctx.add_service({"name": "svc", "url": "https://invalid.example.local/mcp"}, token="BAD_TOKEN")

    # Query service info/metadata
    info = ctx.service_info("svc")
    # Depending on environment, these keys may exist. We just print what's available.
    print("service_info:", info)

    # In real 401/403 cases, metadata.failure_reason == "auth_failed" will be set by the system.


if __name__ == "__main__":
    main()

