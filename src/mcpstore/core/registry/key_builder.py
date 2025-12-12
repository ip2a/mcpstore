from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class KeyBuilder:
    """
    Redis 缓存命名空间的键构建器。
    
    新的键布局使用三层架构：
      {namespace}:entity:{entity_type}:{key}
      {namespace}:relations:{relation_type}:{key}
      {namespace}:state:{state_type}:{key}
    
    命名空间提供不同应用程序/环境之间的隔离。
    默认命名空间是从 mcp.json 路径自动生成的（5字符哈希）。
    """

    namespace: str = "mcpstore"

    def base(self) -> str:
        """返回基础键前缀: mcpstore:{namespace}"""
        return f"mcpstore:{self.namespace}"

