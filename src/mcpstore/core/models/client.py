from typing import Optional, List

from pydantic import BaseModel, Field


class ClientRegistrationRequest(BaseModel):
    client_id: Optional[str] = Field(None, description="Client ID")
    service_names: Optional[List[str]] = Field(None, description="Service name list")

# ClientRegistrationResponse has been moved to common.py, please import directly from common.py
