from typing import Optional

from fastapi import APIRouter, Query

from mcpstore.core.models import ResponseBuilder, timed_response
from mcpstore.api.api_dependencies import get_store
from mcpstore.api.api_decorators import validate_agent_id

router = APIRouter(prefix="/cache", tags=["Mini-Cache"])


def _parse_types(type_param: Optional[str]):
    if not type_param:
        return None
    parts = [t.strip() for t in type_param.split(",") if t.strip()]
    return parts or None


def _summary(data, cache):
    counts = {}
    for item in data or []:
        t = item.get("_type", "unknown")
        counts[t] = counts.get(t, 0) + 1
    return {
        "scope": cache.get_scope() if hasattr(cache, "get_scope") else None,
        "backend": cache.get_backend_type() if hasattr(cache, "get_backend_type") else None,
        "counts": counts,
    }


@router.get("/store/read_entities")
@timed_response
async def store_read_entities(type: Optional[str] = Query(None), key: Optional[str] = None):
    store = get_store()
    cache = store.find_cache()
    types = _parse_types(type)
    data = await cache.read_entity_async(types, key)
    return ResponseBuilder.success(
        message=f"返回 {len(data)} 条实体",
        data={"items": data, "summary": _summary(data, cache)}
    )


@router.get("/store/read_relations")
@timed_response
async def store_read_relations(type: Optional[str] = Query(None), key: Optional[str] = None):
    store = get_store()
    cache = store.find_cache()
    types = _parse_types(type)
    data = await cache.read_relation_async(types, key)
    return ResponseBuilder.success(
        message=f"返回 {len(data)} 条关系",
        data={"items": data, "summary": _summary(data, cache)}
    )


@router.get("/store/read_states")
@timed_response
async def store_read_states(type: Optional[str] = Query(None), key: Optional[str] = None):
    store = get_store()
    cache = store.find_cache()
    types = _parse_types(type)
    data = await cache.read_state_async(types, key)
    return ResponseBuilder.success(
        message=f"返回 {len(data)} 条状态",
        data={"items": data, "summary": _summary(data, cache)}
    )


@router.get("/store/inspect")
@timed_response
async def store_inspect():
    store = get_store()
    cache = store.find_cache()
    data = await cache.inspect_async()
    return ResponseBuilder.success(message="缓存检查", data=data)


@router.get("/store/dump")
@timed_response
async def store_dump():
    store = get_store()
    cache = store.find_cache()
    data = await cache.dump_all_async()
    return ResponseBuilder.success(message="缓存导出", data=data)


@router.get("/agent/{agent_id}/read_entities")
@timed_response
async def agent_read_entities(agent_id: str, type: Optional[str] = Query(None), key: Optional[str] = None):
    validate_agent_id(agent_id)
    store = get_store()
    cache = store.for_agent(agent_id).find_cache()
    types = _parse_types(type)
    data = await cache.read_entity_async(types, key)
    return ResponseBuilder.success(
        message=f"返回 {len(data)} 条实体（agent={agent_id}）",
        data={"items": data, "summary": _summary(data, cache)}
    )


@router.get("/agent/{agent_id}/read_relations")
@timed_response
async def agent_read_relations(agent_id: str, type: Optional[str] = Query(None), key: Optional[str] = None):
    validate_agent_id(agent_id)
    store = get_store()
    cache = store.for_agent(agent_id).find_cache()
    types = _parse_types(type)
    data = await cache.read_relation_async(types, key)
    return ResponseBuilder.success(
        message=f"返回 {len(data)} 条关系（agent={agent_id}）",
        data={"items": data, "summary": _summary(data, cache)}
    )


@router.get("/agent/{agent_id}/read_states")
@timed_response
async def agent_read_states(agent_id: str, type: Optional[str] = Query(None), key: Optional[str] = None):
    validate_agent_id(agent_id)
    store = get_store()
    cache = store.for_agent(agent_id).find_cache()
    types = _parse_types(type)
    data = await cache.read_state_async(types, key)
    return ResponseBuilder.success(
        message=f"返回 {len(data)} 条状态（agent={agent_id}）",
        data={"items": data, "summary": _summary(data, cache)}
    )


@router.get("/agent/{agent_id}/inspect")
@timed_response
async def agent_inspect(agent_id: str):
    validate_agent_id(agent_id)
    store = get_store()
    cache = store.for_agent(agent_id).find_cache()
    data = await cache.inspect_async()
    return ResponseBuilder.success(message=f"缓存检查（agent={agent_id}）", data=data)


@router.get("/agent/{agent_id}/dump")
@timed_response
async def agent_dump(agent_id: str):
    validate_agent_id(agent_id)
    store = get_store()
    cache = store.for_agent(agent_id).find_cache()
    data = await cache.dump_all_async()
    return ResponseBuilder.success(message=f"缓存导出（agent={agent_id}）", data=data)
