import contextlib
from collections.abc import AsyncIterator

import anyio
from mcp import ClientSession
from mcp.shared.memory import create_client_server_memory_streams
from typing_extensions import Unpack

from mcpstore.mcp.client.transports.base import ClientTransport, SessionKwargs
from mcpstore.mcp.server.server import MCPStore as MCPKit


class MCPStoreTransport(ClientTransport):
    """In-memory transport for MCPKit servers.

    This transport connects directly to a MCPKit server instance in the same
    Python process. This is particularly useful for unit tests or scenarios
    where client and server run in the same runtime.
    """

    def __init__(self, mcp: MCPKit, raise_exceptions: bool = False):
        """Initialize a MCPStoreTransport from a MCPKit server instance."""

        self.server = mcp
        self.raise_exceptions = raise_exceptions

    @contextlib.asynccontextmanager
    async def connect_session(
        self, **session_kwargs: Unpack[SessionKwargs]
    ) -> AsyncIterator[ClientSession]:
        async with create_client_server_memory_streams() as (
            client_streams,
            server_streams,
        ):
            client_read, client_write = client_streams
            server_read, server_write = server_streams

            # Capture exceptions to re-raise after task group cleanup.
            # anyio task groups can suppress exceptions when cancel_scope.cancel()
            # is called during cleanup, so we capture and re-raise manually.
            exception_to_raise: BaseException | None = None

            async with (
                anyio.create_task_group() as tg,
                _enter_server_lifespan(server=self.server),
            ):
                tg.start_soon(
                    lambda: self.server._mcp_server.run(
                        server_read,
                        server_write,
                        self.server._mcp_server.create_initialization_options(),
                        raise_exceptions=self.raise_exceptions,
                    )
                )

                try:
                    async with ClientSession(
                        read_stream=client_read,
                        write_stream=client_write,
                        **session_kwargs,
                    ) as client_session:
                        yield client_session
                except BaseException as e:
                    exception_to_raise = e
                finally:
                    tg.cancel_scope.cancel()

            # Re-raise after task group has exited cleanly
            if exception_to_raise is not None:
                raise exception_to_raise

    def __repr__(self) -> str:
        return f"<MCPStoreTransport(server='{self.server.name}')>"


@contextlib.asynccontextmanager
async def _enter_server_lifespan(
    server: MCPKit,
) -> AsyncIterator[None]:
    """进入 MCPKit 的 lifespan 上下文。"""
    async with server._lifespan_manager():
        yield
