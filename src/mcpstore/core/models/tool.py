from pydantic import BaseModel, Field
from typing import Optional, List, Dict, Any
from .common import ListResponse, ExecutionResponse

class ToolInfo(BaseModel):
    name: str
    description: str
    service_name: str
    client_id: Optional[str] = None
    inputSchema: Optional[Dict[str, Any]] = None

class ToolsResponse(BaseModel):
    """工具列表响应模型"""
    tools: List[ToolInfo] = Field(..., description="工具列表")
    total_tools: int = Field(..., description="工具总数")
    success: bool = Field(True, description="操作是否成功")
    message: Optional[str] = Field(None, description="响应消息")

class ToolExecutionRequest(BaseModel):
    tool_name: str = Field(..., description="工具名称")
    args: Dict[str, Any] = Field(..., description="工具参数")
    agent_id: Optional[str] = Field(None, description="Agent ID")
    client_id: Optional[str] = Field(None, description="客户端ID")

# ToolExecutionResponse 已移动到 common.py 中，请直接从 common.py 导入
