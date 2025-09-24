from typing import Optional, List, Dict, Any

from pydantic import BaseModel, Field


class ToolInfo(BaseModel):
    name: str
    description: str
    service_name: str
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
