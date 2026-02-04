from mcpstore.mcp import MCPKit
from mcpstore.mcp.contrib.component_manager import set_up_component_manager
from mcpstore.mcp.server.auth.providers.jwt import JWTVerifier, RSAKeyPair

key_pair = RSAKeyPair.generate()

auth = JWTVerifier(
    public_key=key_pair.public_key,
    issuer="https://dev.example.com",
    audience="my-dev-server",
    required_scopes=["mcp:read"],
)

# Build main server
mcp_token = key_pair.create_token(
    subject="dev-user",
    issuer="https://dev.example.com",
    audience="my-dev-server",
    scopes=["mcp:write", "mcp:read"],
)
mcp = MCPKit(
    name="Component Manager",
    instructions="This is a test server with component manager.",
    auth=auth,
)

# Set up main server component manager
set_up_component_manager(server=mcp, required_scopes=["mcp:write"])

# Build mounted server
mounted_token = key_pair.create_token(
    subject="dev-user",
    issuer="https://dev.example.com",
    audience="my-dev-server",
    scopes=["mounted:write", "mcp:read"],
)
mounted = MCPKit(
    name="Component Manager",
    instructions="This is a test server with component manager.",
    auth=auth,
)

# Set up mounted server component manager
set_up_component_manager(server=mounted, required_scopes=["mounted:write"])

# Mount
mcp.mount(server=mounted, namespace="mo")


@mcp.resource("resource://greeting")
def get_greeting() -> str:
    """Provides a simple greeting message."""
    return "Hello from MCPKit Resources!"


@mounted.tool("greeting")
def get_info() -> str:
    """Provides a simple info."""
    return "You are using component manager contrib module!"
