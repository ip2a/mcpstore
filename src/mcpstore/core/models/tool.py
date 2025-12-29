from typing import Optional, List, Dict, Any

from pydantic import BaseModel, Field


class ToolInfo(BaseModel):
    # 工具全局名称（L3，用于内部索引/调用）
    name: str
    # FastMCP 标准格式名称（L2，用于调用 FastMCP）
    tool_original_name: str
    # 服务原始名称（L0/FastMCP 视角）
    service_original_name: str
    # 服务全局名称（L3，用于内部索引）
    service_global_name: str
    # 为兼容既有接口，service_name 保持与原始名称一致
    service_name: str
    description: str
    client_id: Optional[str] = None
    inputSchema: Optional[Dict[str, Any]] = None

class ToolsResponse(BaseModel):
    """Tool list response model"""
    tools: List[ToolInfo] = Field(..., description="Tool list")
    total_tools: int = Field(..., description="Total number of tools")
    success: bool = Field(True, description="Whether operation was successful")
    message: Optional[str] = Field(None, description="Response message")

class ToolExecutionRequest(BaseModel):
    tool_name: str = Field(..., description="Tool name (FastMCP original name)")
    service_name: str = Field(..., description="Service name")
    args: Dict[str, Any] = Field(default_factory=dict, description="Tool parameters")
    agent_id: Optional[str] = Field(None, description="Agent ID")
    client_id: Optional[str] = Field(None, description="Client ID")
    session_id: Optional[str] = Field(None, description="Session ID (for session-aware execution)")

    # FastMCP standard parameters
    timeout: Optional[float] = Field(None, description="Timeout (seconds)")
    progress_handler: Optional[Any] = Field(None, description="Progress handler")
    raise_on_error: bool = Field(True, description="Whether to raise exception on error")

# ToolExecutionResponse has been moved to common.py, please import directly from common.py
