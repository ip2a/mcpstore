"""
PerspectiveResolver

统一处理 Store/Agent 视角下的服务/工具命名转换与宽容解析。
"""

import logging
from dataclasses import dataclass
from typing import Any, Dict, List, Optional, Tuple

from mcpstore.core.cache.naming_service import NamingService
from mcpstore.core.context.agent_service_mapper import AgentServiceMapper
from mcpstore.core.registry.tool_resolver import ToolNameResolver

logger = logging.getLogger(__name__)


@dataclass
class ServiceResolution:
    """服务命名解析结果"""

    agent_id: str
    local_name: str
    global_name: str
    resolution_method: str
    original_input: str


@dataclass
class ToolResolution:
    """工具命名解析结果"""

    agent_id: str
    local_service_name: str
    global_service_name: str
    local_tool_name: str
    global_tool_name: str
    canonical_tool_name: str  # SDK 标准格式（上游原始命名）
    resolution_method: str
    original_input: str


class PerspectiveResolver:
    """
    视角统一转换器

    - 输入可为本地名/全局名/`agent:service`，输出指定视角。
    - 服务与工具的命名规则均复用 NamingService 与 AgentServiceMapper。
    - 可选 strict 模式：agent 不匹配或格式不合法时直接抛出异常。
    """

    def __init__(self, registry: Any = None):
        """
        Args:
            registry: 可选，用于映射查询（如需要从映射表确认归属）。不依赖时可为空。
        """
        self._registry = registry

    # === 服务相关 ===
    def parse_agent_scoped(self, name: str) -> Tuple[Optional[str], str]:
        """
        解析带 agent 信息的服务标识
        支持：
        - 全局名：<local>_byagent_<agent_id>
        - 冒号分隔：<agent_id>:<local>
        - 普通本地名：返回 (None, name)
        """
        if not name:
            raise ValueError("服务名称不能为空")

        # 全局名格式
        if AgentServiceMapper.is_any_agent_service(name):
            parsed_agent, local_name = AgentServiceMapper.parse_agent_service_name(name)
            return parsed_agent, local_name

        # 冒号分隔格式
        if ":" in name:
            maybe_agent, maybe_local = name.split(":", 1)
            if maybe_agent and maybe_local:
                return maybe_agent, maybe_local

        # 默认为本地名
        return None, name

    def normalize_service_name(
        self,
        agent_id: str,
        name: str,
        *,
        target: str = "global",
        strict: bool = False,
    ) -> ServiceResolution:
        """
        将任意服务标识转换为指定视角。

        Args:
            agent_id: 目标 agent_id
            name: 传入的服务标识（本地/全局/agent:service）
            target: "global" 或 "local"
            strict: 严格模式，agent 不匹配时抛出异常
        """
        if target not in ("global", "local"):
            raise ValueError("target 必须是 'global' 或 'local'")
        if not agent_id:
            raise ValueError("agent_id 不能为空")
        if not name:
            raise ValueError("服务名称不能为空")

        parsed_agent, parsed_local = self.parse_agent_scoped(name)

        # 校验 agent
        if parsed_agent and parsed_agent != agent_id:
            # 特殊：global_agent_store 视角允许看到其他 Agent 的全局名，但不应本地化
            if agent_id == NamingService.GLOBAL_AGENT_STORE:
                # 直接保持全局名
                return ServiceResolution(
                    agent_id=agent_id,
                    local_name=name if target == "local" else name,
                    global_name=name,
                    resolution_method="global_agent_passthrough",
                    original_input=name,
                )
            msg = f"服务归属的 agent_id={parsed_agent} 与目标 agent_id={agent_id} 不一致"
            if strict:
                raise ValueError(msg)
            logger.warning(f"[PERSPECTIVE] {msg}，按目标 agent_id 继续转换")

        mapper = AgentServiceMapper(agent_id)

        # 统一得出本地名
        if AgentServiceMapper.is_any_agent_service(name):
            local_name = mapper.to_local_name(name)
            resolution_method = "parsed_byagent"
        elif parsed_agent:
            local_name = parsed_local
            resolution_method = "agent_prefix"
        else:
            local_name = name
            resolution_method = "assume_local"

        # 计算目标视角
        if target == "global":
            global_name = NamingService.generate_service_global_name(local_name, agent_id)
            final_local = local_name
        else:
            # target == "local"
            if AgentServiceMapper.is_any_agent_service(name):
                final_local = mapper.to_local_name(name)
                global_name = NamingService.generate_service_global_name(final_local, agent_id)
            else:
                final_local = local_name
                global_name = NamingService.generate_service_global_name(final_local, agent_id)

        return ServiceResolution(
            agent_id=agent_id,
            local_name=final_local,
            global_name=global_name,
            resolution_method=resolution_method,
            original_input=name,
        )

    # === 工具相关 ===
    def resolve_tool(
        self,
        agent_id: str,
        user_input: str,
        *,
        available_tools: List[Dict[str, Any]],
        target: str = "canonical",
        strict: bool = False,
    ) -> ToolResolution:
        """
        解析工具名并转换到指定视角。

        Args:
            agent_id: 目标 agent_id
            user_input: 用户输入的工具名（可带服务前缀等）
            available_tools: 可用工具列表（需包含 service_name）
        target: "canonical" | "global" | "local"（影响输出的工具/服务名称形式）
            strict: 严格模式，agent 不匹配时抛出异常
        """
        if target != "canonical":
            raise ValueError("resolve_tool 目前仅支持 target='canonical'，请勿传入其他值")
        if not available_tools:
            raise ValueError("available_tools 不能为空，需提供用于解析的工具列表")

        resolver = ToolNameResolver()
        # 使用当前注册器提供的标准格式解析（上游原始格式，统一称 canonical）
        canonical_name, resolution = resolver.resolve_and_format_for_mcpstore(user_input, available_tools)

        service_local = resolution.service_name
        resolution_method = getattr(resolution, "resolution_method", "resolved")

        # 尝试从 available_tools 补全全局信息；若缺失则回退构造
        matched = next(
            (
                t for t in available_tools
                if t.get("service_name") == service_local
                and (t.get("original_name") or t.get("name") == canonical_name)
            ),
            None,
        )

        service_global = None
        global_tool_name = None
        if matched:
            service_global = matched.get("global_service_name")
            global_tool_name = matched.get("global_tool_name")

        # 若未补全到全局信息，按规则构造；严格模式下缺关键字段抛错
        if not service_global:
            if strict:
                raise ValueError(f"匹配工具缺少全局服务名: service={service_local}, tool={canonical_name}")
            service_global = service_local  # 回退：使用本地名充当全局名

        if not global_tool_name:
            global_tool_name = NamingService.generate_tool_global_name(service_global, canonical_name)

        local_tool_name = f"{service_local}_{canonical_name}"

        return ToolResolution(
            agent_id=agent_id,
            local_service_name=service_local,
            global_service_name=service_global,
            local_tool_name=local_tool_name,
            global_tool_name=global_tool_name,
            canonical_tool_name=canonical_name,
            resolution_method=resolution_method,
            original_input=user_input,
        )

    def to_global_tool_name(self, agent_id: str, tool_original_name: str, service_name: str, strict: bool = False) -> str:
        """
        将工具名转换为全局工具名（使用服务全局名 + 工具原始名）。
        """
        service_res = self.normalize_service_name(agent_id, service_name, target="global", strict=strict)
        return NamingService.generate_tool_global_name(service_res.global_name, tool_original_name)

    def to_local_tool_name(self, agent_id: str, global_tool_name: str, strict: bool = False) -> str:
        """
        将全局工具名转换为本地工具名（本地服务名前缀 + 原始工具名）。
        """
        if not global_tool_name or "_" not in global_tool_name:
            raise ValueError("global_tool_name 格式不正确")
        # 拆分出服务全局名与工具原始名（从右侧拆分，兼容服务名内含下划线）
        parts = global_tool_name.rsplit("_", 1)
        if len(parts) != 2:
            raise ValueError("global_tool_name 解析失败")
        service_global, tool_original = parts
        service_res = self.normalize_service_name(agent_id, service_global, target="local", strict=strict)
        return f"{service_res.local_name}_{tool_original}"
