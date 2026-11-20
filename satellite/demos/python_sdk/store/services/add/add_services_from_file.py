"""Add services to the current Store scope from JSON and TOML files plus an inline config."""

import json
import tempfile
from pathlib import Path

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"


def main() -> None:
    with tempfile.TemporaryDirectory() as directory:
        directory = Path(directory)
        json_path = directory / "services.json"
        toml_path = directory / "services.toml"
        json_path.write_text(
            json.dumps({"mcpServers": {"from-json": {"url": URL}}}), encoding="utf-8"
        )
        toml_path.write_text(
            '[mcpServers.from-toml]\nurl = "http://115.190.220.103:2230/mcp"\n',
            encoding="utf-8",
        )

        store = MCPStore.setup_store(debug=False)
        context = store.for_store()
        context.reset_config()
        try:
            context = context.add_service(json_path)
            context = context.add_service(str(toml_path))
            context = context.add_service({"name": "inline", "url": URL})
            assert {service.info()["service_name"] for service in context.list_services()} == {
                "from-json",
                "from-toml",
                "inline",
            }
            print("config=", context.show_config())
        finally:
            context.reset_config()


if __name__ == "__main__":
    main()
