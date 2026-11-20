"""Special topic: add a service with structured OAuth configuration and verify it through the current scope's config."""

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    context = store.for_store()
    context.reset_config()
    try:
        context.add_service(
            {
                "mcpServers": {
                    "protected-weather": {
                        "url": URL,
                        "auth": {
                            "type": "oauth_authorization_code",
                            "client_id": "example-client",
                            "scopes": ["mcp:read"],
                        },
                    }
                }
            }
        )
        config = context.show_config()
        auth = config["mcpServers"]["protected-weather"]["auth"]
        assert auth["type"] == "oauth_authorization_code"
        assert auth["client_id"] == "example-client"
        print("auth_config=", auth)
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
