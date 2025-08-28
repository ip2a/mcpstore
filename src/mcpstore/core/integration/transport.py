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
            Dict[str, Any]: 服务器的初始化响应
        """
        headers = {
            "Accept": "application/json, text/event-stream",
            "Content-Type": "application/json"
        }
        
        request_id = str(uuid.uuid4())
        # 确保使用正确的方法名（initialize 不需要映射，但为了一致性，我们仍然从映射中获取）
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
                "protocolVersion": "2024-11-05",  # 🔧 修复：使用标准MCP协议版本
                "capabilities": {                 # 🔧 修复：使用标准MCP能力格式
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
            
            # 获取并保存会话ID
            session_id = response.headers.get(self.config.session_id_header)
            if session_id:
                self.config.session_id = session_id
                logger.info(f"Session established with ID: {session_id}")
            
            # 处理响应内容
            if response.content:
                try:
                    return response.json()
                except json.JSONDecodeError:
                    logger.warning(f"Failed to parse response as JSON: {response.content}")
                    # 返回一个默认的成功响应，避免中断流程
                    return {"status": "connected", "session_id": session_id or "unknown"}
            else:
                logger.warning("Empty response received from server")
                # 返回一个默认的成功响应，避免中断流程
                return {"status": "connected", "session_id": session_id or "unknown"}
            
        except httpx.HTTPStatusError as e:
            logger.error(f"HTTP error during initialization: {e.response.status_code} {e.response.reason_phrase}")
            raise
        except Exception as e:
            logger.error(f"Error during transport initialization: {e}")
            raise
    
    async def call_tool(self, tool_name: str, tool_args: Dict[str, Any]) -> Any:
        """调用工具方法
        
        使用Streamable HTTP协议调用指定的工具，并返回结果。
        此方法与registry.py中定义的SessionProtocol接口兼容。
        
        Args:
            tool_name: 工具名称
            tool_args: 工具参数
            
        Returns:
            Any: 工具执行结果
        """
        logger.info(f"Calling tool '{tool_name}' with args: {tool_args}")
        
        try:
            # 发送工具调用请求
            responses = []
            # 使用 call_tool 作为方法名，会被映射到 tools/call
            method = "call_tool"
            params = {"name": tool_name, "arguments": tool_args}
            
            async for response in self.send_request(method, params):
                responses.append(response)
                # 只获取第一个响应
                break
                
            if not responses:
                logger.warning(f"No response received from tool '{tool_name}'")
                return {"content": [{"text": f"No response received from tool '{tool_name}'"}]}
                
            result = responses[0]
            
            # 格式化响应为兼容格式
            if isinstance(result, dict) and "result" in result:
                # 如果响应中有result字段，将其作为文本内容返回
                return {"content": [{"text": str(result["result"])}]}
            elif isinstance(result, dict) and "error" in result:
                # 如果响应中有error字段，将其作为错误信息返回
                error_msg = result.get("error", {}).get("message", "Unknown error")
                return {"content": [{"text": f"Error: {error_msg}"}]}
            else:
                # 其他情况，直接返回响应
                return {"content": [{"text": str(result)}]}
                
        except Exception as e:
            logger.error(f"Error calling tool '{tool_name}': {e}", exc_info=True)
            return {"content": [{"text": f"Error calling tool '{tool_name}': {str(e)}"}]}
        
    async def send_request(self, method: str, params: Dict[str, Any]) -> AsyncGenerator[Dict[str, Any], None]:
        """发送请求并处理流式响应
        
        Args:
            method: 请求方法名
            params: 请求参数
            
        Yields:
            Dict[str, Any]: 服务器响应数据流
        """
        headers = {
            "Accept": "application/json, text/event-stream",
            "Content-Type": "application/json"
        }
        
        if self.config.session_id:
            headers[self.config.session_id_header] = self.config.session_id
            
        if self.last_event_id:
            headers[self.config.event_id_header] = self.last_event_id
        
        # 将简化的方法名转换为服务器期望的格式
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
                    # 处理SSE流
                    buffer = ""
                    async for chunk in response.aiter_text():
                        buffer += chunk
                        
                        while "\n\n" in buffer:
                            message, buffer = buffer.split("\n\n", 1)
                            event_data = {}
                            
                            for line in message.split("\n"):
                                if not line or line.startswith(":"):
                                    continue  # 忽略注释和空行
                                    
                                if ":" in line:
                                    field, value = line.split(":", 1)
                                    value = value.lstrip()  # 移除前导空格
                                    
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
                    # 处理普通JSON响应 - 修复方法，读取完整响应内容
                    try:
                        # 读取完整响应内容而不是直接调用response.json()
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
        """发送通知（不需要响应的请求）
        
        Args:
            method: 通知方法名
            params: 通知参数
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
        """监听服务器发送的消息
        
        打开GET连接以接收服务器主动发送的消息。
        
        Yields:
            Dict[str, Any]: 服务器发送的消息
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
                                continue  # 忽略注释和空行
                                
                            if ":" in line:
                                field, value = line.split(":", 1)
                                value = value.lstrip()  # 移除前导空格
                                
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
        """关闭连接并清理资源
        
        如果有会话ID，尝试显式终止会话。
        """
        if self.config.session_id:
            try:
                headers = {self.config.session_id_header: self.config.session_id}
                await self.client.delete(
                    urljoin(self.config.base_url, "/mcp"),
                    headers=headers
                )
                logger.info(f"Session {self.config.session_id} terminated")
            except Exception as e:
                logger.warning(f"Failed to terminate session: {e}")
                
        await self.client.aclose()
        logger.info("Transport resources cleaned up")

