import os
import sys

sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from dataclasses import dataclass
from typing import Dict, Any, Optional, AsyncGenerator
import uuid
import httpx
import json
import logging
from urllib.parse import urljoin

logger = logging.getLogger(__name__)

@dataclass
class StreamableHTTPConfig:
    """Streamable HTTP transport configuration"""
    base_url: str
    timeout: int = 30
    session_id: Optional[str] = None
    retry_attempts: int = 3
    retry_delay: float = 1.0
    event_id_header: str = "Last-Event-ID"
    session_id_header: str = "Mcp-Session-Id"

class StreamableHTTPTransport:
    """Implements MCP Streamable HTTP transport protocol

    Based on MCP 2025-03-26 version specification, providing unified bidirectional communication capabilities.
    Supports session management, connection recovery and backward compatibility.
    """
    
    # Method name mapping, mapping simplified names to server-expected format
    METHOD_MAPPING = {
        "list_tools": "tools/list",
        "call_tool": "tools/call",
        "initialize": "initialize",
        "ping": "ping"
        # More mappings can be added as needed
    }
    
    def __init__(self, config: StreamableHTTPConfig):
        self.config = config
        self.client = httpx.AsyncClient(timeout=config.timeout, follow_redirects=True)
        self.last_event_id: Optional[str] = None
        
    async def initialize(self) -> Dict[str, Any]:
        """Initialize connection and get session ID

        Send initialization request, establish session, and return server response.
        
        Returns:
            Dict[str, Any]: Server initialization response
        """
        headers = {
            "Accept": "application/json, text/event-stream",
            "Content-Type": "application/json"
        }
        
        request_id = str(uuid.uuid4())
        # Ensure using correct method name (initialize doesn't need mapping, but for consistency we still get from mapping)
        method = "initialize"
        server_method = self.METHOD_MAPPING.get(method, method)
        
        payload = {
            "jsonrpc": "2.0",
            "method": server_method,
            "params": {
                "clientInfo": {
                    "name": "mcp-client",
                    "version": "1.0.0"
                },
                "protocolVersion": "2024-11-05",  #  Fixed: Use standard MCP protocol version
                "capabilities": {                 #  Fixed: Use standard MCP capability format
                    "tools": {}
                }
            },
            "id": request_id
        }
        
        try:
            logger.debug(f"Initializing connection with method={server_method}")
            response = await self.client.post(
                urljoin(self.config.base_url, "/mcp"),
                headers=headers,
                json=payload
            )
            response.raise_for_status()
            
            # Get and save session ID
            session_id = response.headers.get(self.config.session_id_header)
            if session_id:
                self.config.session_id = session_id
                logger.debug(f"Session established with ID: {session_id}")
            
            # Handle response content
            if response.content:
                try:
                    return response.json()
                except json.JSONDecodeError:
                    logger.warning(f"Failed to parse response as JSON: {response.content}")
                    # Return a default success response to avoid interrupting the process
                    return {"status": "connected", "session_id": session_id or "unknown"}
            else:
                logger.warning("Empty response received from server")
                # Return a default success response to avoid interrupting the process
                return {"status": "connected", "session_id": session_id or "unknown"}
            
        except httpx.HTTPStatusError as e:
            logger.error(f"HTTP error during initialization: {e.response.status_code} {e.response.reason_phrase}")
            raise
        except Exception as e:
            logger.error(f"Error during transport initialization: {e}")
            raise
    
    async def call_tool(self, tool_name: str, tool_args: Dict[str, Any]) -> Any:
        """Call tool method

        Use Streamable HTTP protocol to call specified tool and return result.
        This method is compatible with SessionProtocol interface defined in registry.py.

        Args:
            tool_name: Tool name
            tool_args: Tool arguments

        Returns:
            Any: Tool execution result
        """
        logger.debug(f"Calling tool '{tool_name}' with args: {type(tool_args).__name__}")
        
        try:
            # Send tool call request
            responses = []
            # Use call_tool as method name, will be mapped to tools/call
            method = "call_tool"
            params = {"name": tool_name, "arguments": tool_args}
            
            async for response in self.send_request(method, params):
                responses.append(response)
                # Only get first response
                break
                
            if not responses:
                logger.warning(f"No response received from tool '{tool_name}'")
                return {"content": [{"text": f"No response received from tool '{tool_name}'"}]}
                
            result = responses[0]
            
            # Format response for compatibility
            if isinstance(result, dict) and "result" in result:
                # If response has result field, return it as text content
                return {"content": [{"text": str(result["result"])}]}
            elif isinstance(result, dict) and "error" in result:
                # If response has error field, return it as error message
                error_msg = result.get("error", {}).get("message", "Unknown error")
                return {"content": [{"text": f"Error: {error_msg}"}]}
            else:
                # Other cases, return response directly
                return {"content": [{"text": str(result)}]}
                
        except Exception as e:
            logger.error(f"Error calling tool '{tool_name}': {e}", exc_info=True)
            return {"content": [{"text": f"Error calling tool '{tool_name}': {str(e)}"}]}
        
    async def send_request(self, method: str, params: Dict[str, Any]) -> AsyncGenerator[Dict[str, Any], None]:
        """Send request and handle streaming response

        Args:
            method: Request method name
            params: Request parameters

        Yields:
            Dict[str, Any]: Server response data stream
        """
        headers = {
            "Accept": "application/json, text/event-stream",
            "Content-Type": "application/json"
        }
        
        if self.config.session_id:
            headers[self.config.session_id_header] = self.config.session_id
            
        if self.last_event_id:
            headers[self.config.event_id_header] = self.last_event_id
        
        # Convert simplified method name to server-expected format
        server_method = self.METHOD_MAPPING.get(method, method)
        if server_method != method:
            logger.debug(f"Mapping method name from '{method}' to '{server_method}'")
            
        request_id = str(uuid.uuid4())
        payload = {
            "jsonrpc": "2.0",
            "method": server_method,
            "params": params,
            "id": request_id
        }
        
        try:
            logger.debug(f"Sending request: method={server_method}, params={params}")
            async with self.client.stream(
                "POST",
                urljoin(self.config.base_url, "/mcp"),
                headers=headers,
                json=payload
            ) as response:
                response.raise_for_status()
                
                content_type = response.headers.get("Content-Type", "")
                
                if "text/event-stream" in content_type:
                    # Handle SSE stream
                    buffer = ""
                    async for chunk in response.aiter_text():
                        buffer += chunk
                        
                        while "\n\n" in buffer:
                            message, buffer = buffer.split("\n\n", 1)
                            event_data = {}
                            
                            for line in message.split("\n"):
                                if not line or line.startswith(":"):
                                    continue  # Ignore comments and empty lines
                                    
                                if ":" in line:
                                    field, value = line.split(":", 1)
                                    value = value.lstrip()  # Remove leading spaces
                                    
                                    if field == "id":
                                        self.last_event_id = value
                                    elif field == "data":
                                        try:
                                            event_data = json.loads(value)
                                        except json.JSONDecodeError:
                                            logger.warning(f"Failed to parse SSE data: {value}")
                            
                            if event_data:
                                yield event_data
                else:
                    # Handle regular JSON response - Fixed method, read complete response content
                    try:
                        # Read complete response content instead of calling response.json() directly
                        content = await response.aread()
                        data = json.loads(content)
                        yield data
                    except json.JSONDecodeError:
                        logger.error(f"Failed to parse response as JSON: {content}")
                        raise
                    
        except httpx.HTTPStatusError as e:
            logger.error(f"HTTP error during request: {e.response.status_code} {e.response.reason_phrase}")
            raise
        except Exception as e:
            logger.error(f"Error during request processing: {e}")
            raise
    
    async def send_notification(self, method: str, params: Dict[str, Any]) -> None:
        """Send notification (request that doesn't need response)

        Args:
            method: Notification method name
            params: Notification parameters
        """
        headers = {
            "Accept": "application/json",
            "Content-Type": "application/json"
        }
        
        if self.config.session_id:
            headers[self.config.session_id_header] = self.config.session_id
            
        payload = {
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        }
        
        try:
            response = await self.client.post(
                urljoin(self.config.base_url, "/mcp"),
                headers=headers,
                json=payload
            )
            
            if response.status_code != 202:
                logger.warning(f"Unexpected status code for notification: {response.status_code}")
                
        except Exception as e:
            logger.error(f"Error sending notification: {e}")
            raise
    
    async def listen_server(self) -> AsyncGenerator[Dict[str, Any], None]:
        """Listen for messages sent by server

        Open GET connection to receive messages actively sent by server.

        Yields:
            Dict[str, Any]: Messages sent by server
        """
        headers = {
            "Accept": "text/event-stream"
        }
        
        if self.config.session_id:
            headers[self.config.session_id_header] = self.config.session_id
            
        if self.last_event_id:
            headers[self.config.event_id_header] = self.last_event_id
        
        try:
            async with self.client.stream(
                "GET",
                urljoin(self.config.base_url, "/mcp"),
                headers=headers
            ) as response:
                response.raise_for_status()
                
                if response.status_code == 405:
                    logger.warning("Server does not support GET requests for listening")
                    return
                
                buffer = ""
                async for chunk in response.aiter_text():
                    buffer += chunk
                    
                    while "\n\n" in buffer:
                        message, buffer = buffer.split("\n\n", 1)
                        event_data = {}
                        
                        for line in message.split("\n"):
                            if not line or line.startswith(":"):
                                continue  # Ignore comments and empty lines
                                
                            if ":" in line:
                                field, value = line.split(":", 1)
                                value = value.lstrip()  # Remove leading spaces
                                
                                if field == "id":
                                    self.last_event_id = value
                                elif field == "data":
                                    try:
                                        event_data = json.loads(value)
                                    except json.JSONDecodeError:
                                        logger.warning(f"Failed to parse SSE data: {value}")
                        
                        if event_data:
                            yield event_data
                            
        except httpx.HTTPStatusError as e:
            if e.response.status_code == 405:
                logger.warning("Server does not support GET requests for listening")
            else:
                logger.error(f"HTTP error during listening: {e.response.status_code} {e.response.reason_phrase}")
            raise
        except Exception as e:
            logger.error(f"Error during server listening: {e}")
            raise
                
    async def close(self) -> None:
        """Close connection and cleanup resources

        If session ID exists, try to explicitly terminate session.
        """
        if self.config.session_id:
            try:
                headers = {self.config.session_id_header: self.config.session_id}
                await self.client.delete(
                    urljoin(self.config.base_url, "/mcp"),
                    headers=headers
                )
                logger.debug(f"Session {self.config.session_id} terminated")
            except Exception as e:
                logger.warning(f"Failed to terminate session: {e}")
                
        await self.client.aclose()
        logger.debug("Transport resources cleaned up")

