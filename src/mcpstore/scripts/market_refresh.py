import asyncio
from typing import Optional

from mcpstore.core.store import MCPStore


async def refresh_market(remote_url: Optional[str] = None, force: bool = False) -> bool:
    """Manual market remote refresh helper.
    - If remote_url provided, adds it as remote source before refresh
    - Returns True if any remote source merged successfully
    """
    store = MCPStore.setup_store(debug=False)
    if remote_url:
        store._market_manager.add_remote_source(remote_url)
    return await store._market_manager.refresh_from_remote_async(force=force)


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--remote-url", type=str, default=None)
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()
    ok = asyncio.run(refresh_market(args.remote_url, args.force))
    print("refreshed:" if ok else "no-change or skipped")

