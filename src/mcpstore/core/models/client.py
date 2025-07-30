from typing import Optional, List

from pydantic import BaseModel, Field


class ClientRegistrationRequest(BaseModel):
    client_id: Optional[str] = Field(None, description="客户端ID")
    service_names: Optional[List[str]] = Field(None, description="服务名列表")

# ClientRegistrationResponse 已移动到 common.py 中，请直接从 common.py 导入
