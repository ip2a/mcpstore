"""
应用层模块

包含应用服务，协调领域服务完成用户请求：
- ServiceApplicationService: 服务应用服务
"""

from .service_application_service import ServiceApplicationService, AddServiceResult

__all__ = [
    "ServiceApplicationService",
    "AddServiceResult",
]

