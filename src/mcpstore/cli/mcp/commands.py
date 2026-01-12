"""
MCP 命令组：服务添加/查询/删除
对齐 Claude 风格语法，增加 --for-agent 作用域切换与自动推断 transport。
"""

import json
from typing import Any, Dict, List, Optional

import typer

from mcpstore import MCPStore

TransportLiteral = str
_STORE_SINGLETON: Optional[MCPStore] = None


def register_mcp_commands(app: typer.Typer) -> None:
    """向主 Typer 实例注册 MCP 相关命令"""

    @app.command("add")
    def add_service(
        name: Optional[str] = typer.Argument(
            None, help="服务名称，或直接传入 JSON（以 { 或 [ 开头）"
        ),
        command_or_url: Optional[str] = typer.Argument(
            None, help="命令或 URL；也可传 JSON"
        ),
        args: List[str] = typer.Argument(
            None, help="命令参数（仅 stdio 模式使用）"
        ),
        transport: Optional[str] = typer.Option(
            None,
            "--transport",
            help="传输方式：stdio/http/sse（缺省时自动推断）",
        ),
        for_agent: Optional[str] = typer.Option(
            None, "--for-agent", help="目标 Agent ID，缺省作用于 Store"
        ),
        env: List[str] = typer.Option(
            None,
            "--env",
            "-e",
            help="环境变量或请求头，格式 KEY=VAL，可重复",
        ),
    ):
        """添加服务，支持 stdio/http/sse 及 JSON 配置"""
        try:
            args = args or []
            env_map = _parse_env(env)
            config, resolved_name, resolved_transport = _build_config_from_inputs(
                name=name,
                command_or_url=command_or_url,
                args=args,
                transport=transport,
                env_map=env_map,
            )

            store = _get_store_singleton()
            ctx = store.for_agent(for_agent) if for_agent else store.for_store()

            ctx.add_service(config)
            scope = f"agent:{for_agent}" if for_agent else "store"
            typer.echo(f"[成功] 已添加服务: {resolved_name} 作用域={scope} 传输={resolved_transport}")
        except Exception as e:
            typer.echo(f"[错误] 添加服务失败: {e}")
            raise typer.Exit(1)

    @app.command("list")
    def list_services(
        for_agent: Optional[str] = typer.Option(
            None, "--for-agent", help="目标 Agent ID，缺省作用于 Store"
        )
    ):
        """列出服务"""
        try:
            store = _get_store_singleton()
            ctx = store.for_agent(for_agent) if for_agent else store.for_store()
            services = ctx.list_services() or []

            scope = f"agent:{for_agent}" if for_agent else "store"
            typer.echo(f"[列表] 作用域={scope} 服务数={len(services)}")

            if not services:
                typer.echo("  暂无服务")
                return

            for svc in services:
                svc_dict = _model_to_dict(svc)
                name = svc_dict.get("name") or svc_dict.get("service_name") or "未知"
                transport = (
                    svc_dict.get("transport_type")
                    or svc_dict.get("transport")
                    or ""
                )
                status_raw = svc_dict.get("status") or svc_dict.get("state") or ""
                status = status_raw.value if hasattr(status_raw, "value") else status_raw
                client_id = svc_dict.get("client_id") or ""
                typer.echo(
                    f"- {name}  transport={transport}  status={status}  client_id={client_id}"
                )
        except Exception as e:
            typer.echo(f"[错误] 列出服务失败: {e}")
            raise typer.Exit(1)

    @app.command("get")
    def get_service(
        name: str = typer.Argument(..., help="服务名称"),
        for_agent: Optional[str] = typer.Option(
            None, "--for-agent", help="目标 Agent ID，缺省作用于 Store"
        ),
    ):
        """查看服务详情"""
        try:
            store = _get_store_singleton()
            ctx = store.for_agent(for_agent) if for_agent else store.for_store()
            proxy = ctx.find_service(name)
            info = _model_to_dict(proxy.service_info())
            status = _safe_status(proxy)
            tools = proxy.list_tools() if hasattr(proxy, "list_tools") else []
            data = {
                "service": info,
                "status": status,
                "tools": [_model_to_dict(t) for t in tools],
            }
            typer.echo(json.dumps(data, ensure_ascii=False, indent=2))
        except Exception as e:
            typer.echo(f"[错误] 获取服务详情失败: {e}")
            raise typer.Exit(1)

    @app.command("remove")
    def remove_service(
        name: str = typer.Argument(..., help="服务名称"),
        for_agent: Optional[str] = typer.Option(
            None, "--for-agent", help="目标 Agent ID，缺省作用于 Store"
        ),
    ):
        """删除服务"""
        try:
            store = _get_store_singleton()
            ctx = store.for_agent(for_agent) if for_agent else store.for_store()
            proxy = ctx.find_service(name)
            ok = proxy.delete_service()
            if not ok:
                raise RuntimeError("删除返回失败")
            scope = f"agent:{for_agent}" if for_agent else "store"
            typer.echo(f"[成功] 已删除服务: {name} 作用域={scope}")
        except Exception as e:
            typer.echo(f"[错误] 删除服务失败: {e}")
            raise typer.Exit(1)


# ===== 内部工具函数 =====

def _parse_env(env: Optional[List[str]]) -> Dict[str, str]:
    """解析 --env KEY=VAL 列表"""
    env_map: Dict[str, str] = {}
    if not env:
        return env_map
    for item in env:
        if "=" not in item or item.startswith("="):
            raise ValueError(f"环境变量格式错误: {item}")
        key, val = item.split("=", 1)
        key = key.strip()
        if not key:
            raise ValueError(f"环境变量键不能为空: {item}")
        env_map[key] = val
    return env_map


def _looks_like_json(text: Optional[str]) -> bool:
    """判定字符串是否像 JSON"""
    if not text:
        return False
    stripped = text.lstrip()
    return stripped.startswith("{") or stripped.startswith("[")


def _infer_transport(
    explicit: Optional[str],
    config: Dict[str, Any],
    command_or_url: Optional[str],
    args: List[str],
) -> TransportLiteral:
    """推断 transport，优先使用显式值"""
    if explicit:
        value = explicit.lower()
        if value == "http":
            return "streamable-http"
        if value == "stdio":
            return "stdio"
        if value == "sse":
            return "sse"
        return value

    # JSON 中已有
    transport_in_cfg = config.get("transport")
    if isinstance(transport_in_cfg, str):
        value = transport_in_cfg.lower()
        if value == "http":
            return "streamable-http"
        if value == "sse":
            return "sse"
        return value

    # JSON 内提供 url 但未声明 transport，默认 http
    if isinstance(config.get("url"), str):
        return "streamable-http"

    # 根据字段推断
    if "command" in config or args:
        return "stdio"
    if command_or_url and command_or_url.startswith(("http://", "https://")):
        return "streamable-http"

    raise ValueError("无法推断 transport，请使用 --transport 明确指定（stdio/http/sse）")


def _build_config_from_inputs(
    name: Optional[str],
    command_or_url: Optional[str],
    args: List[str],
    transport: Optional[str],
    env_map: Dict[str, str],
) -> (Dict[str, Any], str, TransportLiteral):
    """
    根据 CLI 输入构建服务配置，返回 (config, name, transport)
    """
    # 1) JSON 直接输入（作为 name 或 command_or_url）
    if _looks_like_json(name):
        cfg = _load_json(name)
        resolved_name = cfg.get("name") or ""
        if not resolved_name:
            raise ValueError("JSON 中缺少 name 字段")
        resolved_transport = _infer_transport(transport, cfg, command_or_url, args)
        return _merge_env_into_config(cfg, env_map, resolved_transport), resolved_name, resolved_transport

    if _looks_like_json(command_or_url):
        cfg = _load_json(command_or_url)
        resolved_name = name or cfg.get("name") or ""
        if not resolved_name:
            raise ValueError("JSON 中缺少 name 字段，且未提供名称参数")
        if "name" not in cfg:
            cfg["name"] = resolved_name
        resolved_transport = _infer_transport(transport, cfg, command_or_url, args)
        return _merge_env_into_config(cfg, env_map, resolved_transport), resolved_name, resolved_transport

    # 2) 非 JSON：按位置参数推断
    if not name:
        raise ValueError("必须提供服务名称")
    if not command_or_url:
        raise ValueError("必须提供 URL 或命令")

    base_config: Dict[str, Any] = {"name": name}
    resolved_transport = _infer_transport(transport, base_config, command_or_url, args)

    if resolved_transport == "stdio":
        base_config.update({
            "command": command_or_url,
            "args": args or [],
            "transport": resolved_transport,
        })
        if env_map:
            base_config["env"] = env_map
    else:
        base_config.update({
            "url": command_or_url,
            "transport": resolved_transport,
        })
        if env_map:
            base_config["headers"] = env_map

    return base_config, name, resolved_transport


def _load_json(text: str) -> Dict[str, Any]:
    """加载 JSON 字符串为字典"""
    try:
        cfg = json.loads(text)
        if not isinstance(cfg, dict):
            raise ValueError("JSON 必须是对象类型")
        return cfg
    except Exception as e:
        raise ValueError(f"JSON 解析失败: {e}")


def _merge_env_into_config(
    cfg: Dict[str, Any],
    env_map: Dict[str, str],
    transport: TransportLiteral,
) -> Dict[str, Any]:
    """合并 env/headers：命令行优先"""
    merged = dict(cfg)
    if transport == "stdio":
        existing = merged.get("env") or {}
        merged["env"] = {**existing, **env_map} if env_map else existing
        # 保留 headers，避免用户在 JSON 中自带 header 配置被丢弃
        if "headers" in cfg:
            merged["headers"] = cfg.get("headers")
    else:
        existing_headers = merged.get("headers") or {}
        merged["headers"] = {**existing_headers, **env_map} if env_map else existing_headers
        # 保留 env，若用户提供（对某些客户端有用）
        if "env" in cfg:
            merged["env"] = cfg.get("env")
    if "transport" not in merged:
        merged["transport"] = transport
    return merged


def _model_to_dict(obj: Any) -> Dict[str, Any]:
    """将 ServiceInfo 或普通对象转为 dict"""
    if obj is None:
        return {}
    if hasattr(obj, "model_dump"):
        return obj.model_dump(exclude_none=True)
    if isinstance(obj, dict):
        return obj
    return {k: v for k, v in obj.__dict__.items() if not k.startswith("_")}


def _safe_status(proxy: Any) -> Dict[str, Any]:
    """获取状态，容错返回字典"""
    try:
        status = proxy.service_status()
        return _model_to_dict(status)
    except Exception:
        return {}


def _get_store_singleton() -> MCPStore:
    """懒加载单例 Store，避免多次 setup"""
    global _STORE_SINGLETON
    if _STORE_SINGLETON is None:
        _STORE_SINGLETON = MCPStore.setup_store()
    return _STORE_SINGLETON
