import logging
from datetime import datetime
from typing import Any, Dict, List, Optional, Tuple, TYPE_CHECKING

from mcpstore.core.models.service import ServiceConnectionState, ServiceStateMetadata

if TYPE_CHECKING:
    from key_value.aio.protocols import AsyncKeyValue


logger = logging.getLogger(__name__)


class RegistryStateBackend:
    """Abstract KV-backed state backend for ServiceRegistry.

    This interface is intentionally minimal and async, matching the underlying
    AsyncKeyValue contract. It is not meant to expose collections/keys to
    callers; higher layers should only depend on these methods.
    """

    async def get_service_state(self, agent_id: str, service_name: str) -> Optional[ServiceConnectionState]:  # pragma: no cover - interface only
        raise NotImplementedError

    async def set_service_state(self, agent_id: str, service_name: str, state: ServiceConnectionState) -> None:  # pragma: no cover - interface only
        raise NotImplementedError

    async def delete_service_state(self, agent_id: str, service_name: str) -> None:  # pragma: no cover - interface only
        raise NotImplementedError

    async def get_service_metadata(self, agent_id: str, service_name: str) -> Optional[ServiceStateMetadata]:  # pragma: no cover - interface only
        raise NotImplementedError

    async def set_service_metadata(self, agent_id: str, service_name: str, metadata: ServiceStateMetadata) -> None:  # pragma: no cover - interface only
        raise NotImplementedError

    async def delete_service_metadata(self, agent_id: str, service_name: str) -> None:  # pragma: no cover - interface only
        raise NotImplementedError

    async def get_tool(self, agent_id: str, tool_name: str) -> Optional[Dict[str, Any]]:  # pragma: no cover - interface only
        raise NotImplementedError

    async def set_tool(self, agent_id: str, tool_name: str, tool_def: Dict[str, Any]) -> None:  # pragma: no cover - interface only
        raise NotImplementedError

    async def delete_tool(self, agent_id: str, tool_name: str) -> None:  # pragma: no cover - interface only
        raise NotImplementedError

    async def list_tools(self, agent_id: str) -> Dict[str, Dict[str, Any]]:  # pragma: no cover - interface only
        raise NotImplementedError

    async def get_tool_service(self, agent_id: str, tool_name: str) -> Optional[str]:  # pragma: no cover - interface only
        raise NotImplementedError

    async def set_tool_service(self, agent_id: str, tool_name: str, service_name: str) -> None:  # pragma: no cover - interface only
        raise NotImplementedError

    async def delete_tool_service(self, agent_id: str, tool_name: str) -> None:  # pragma: no cover - interface only
        raise NotImplementedError

    async def get_service_client(self, agent_id: str, service_name: str) -> Optional[str]:  # pragma: no cover - interface only
        raise NotImplementedError

    async def set_service_client(self, agent_id: str, service_name: str, client_id: str) -> None:  # pragma: no cover - interface only
        raise NotImplementedError

    async def delete_service_client(self, agent_id: str, service_name: str) -> None:  # pragma: no cover - interface only
        raise NotImplementedError

    async def list_agent_clients(self, agent_id: str) -> List[str]:  # pragma: no cover - interface only
        raise NotImplementedError

    async def get_client_config(self, client_id: str) -> Optional[Dict[str, Any]]:  # pragma: no cover - interface only
        raise NotImplementedError

    async def set_client_config(self, client_id: str, config: Dict[str, Any]) -> None:  # pragma: no cover - interface only
        raise NotImplementedError

    async def delete_client_config(self, client_id: str) -> None:  # pragma: no cover - interface only
        raise NotImplementedError


class KVRegistryStateBackend(RegistryStateBackend):
    """Concrete implementation of RegistryStateBackend on top of AsyncKeyValue.

    This class encapsulates collection/key mapping and (de-)serialization
    details. It does not implement any in-memory mirroring; callers that
    require caching should layer it on top as an implementation detail.
    """

    def __init__(self, kv_store: "AsyncKeyValue") -> None:
        self._kv_store = kv_store

    @staticmethod
    def _collection(agent_id: str, data_type: str) -> str:
        """Generate collection name consistent with ServiceRegistry._get_collection."""
        return f"agent:{agent_id}:{data_type}"

    # Service state -------------------------------------------------------

    async def get_service_state(self, agent_id: str, service_name: str) -> Optional[ServiceConnectionState]:
        collection = self._collection(agent_id, "states")
        state_data = await self._kv_store.get(key=service_name, collection=collection)
        if state_data is None:
            return None

        if isinstance(state_data, dict):
            state_value = state_data.get("state")
        else:
            state_value = state_data

        if isinstance(state_value, str):
            try:
                return ServiceConnectionState(state_value)
            except Exception:
                logger.warning(f"Invalid service state value for {agent_id}/{service_name}: {state_value}")
                return None
        if isinstance(state_value, ServiceConnectionState):
            return state_value

        logger.warning(f"Invalid state data for {agent_id}/{service_name}: {state_data}")
        return None

    async def set_service_state(self, agent_id: str, service_name: str, state: ServiceConnectionState) -> None:
        collection = self._collection(agent_id, "states")
        state_data = {
            "state": state.value if hasattr(state, "value") else str(state),
            "updated_at": datetime.now().isoformat(),
        }
        await self._kv_store.put(key=service_name, value=state_data, collection=collection)

    async def delete_service_state(self, agent_id: str, service_name: str) -> None:
        collection = self._collection(agent_id, "states")
        await self._kv_store.delete(key=service_name, collection=collection)

    # Service metadata ----------------------------------------------------

    async def get_service_metadata(self, agent_id: str, service_name: str) -> Optional[ServiceStateMetadata]:
        collection = self._collection(agent_id, "metadata")
        metadata_data = await self._kv_store.get(key=service_name, collection=collection)
        if metadata_data is None:
            return None
        if not isinstance(metadata_data, dict):
            logger.warning(f"Invalid metadata format for {agent_id}/{service_name}")
            return None

        state_entered_time = metadata_data.get("state_entered_time")
        if isinstance(state_entered_time, str):
            state_entered_time = datetime.fromisoformat(state_entered_time)

        last_ping_time = metadata_data.get("last_ping_time")
        if isinstance(last_ping_time, str):
            last_ping_time = datetime.fromisoformat(last_ping_time)

        return ServiceStateMetadata(
            service_name=metadata_data.get("service_name", service_name),
            agent_id=metadata_data.get("agent_id", agent_id),
            state_entered_time=state_entered_time or datetime.now(),
            service_config=metadata_data.get("service_config", {}),
            consecutive_failures=metadata_data.get("consecutive_failures", 0),
            error_message=metadata_data.get("error_message"),
            last_ping_time=last_ping_time,
        )

    async def set_service_metadata(self, agent_id: str, service_name: str, metadata: ServiceStateMetadata) -> None:
        collection = self._collection(agent_id, "metadata")
        metadata_data = {
            "service_name": metadata.service_name,
            "agent_id": metadata.agent_id,
            "state_entered_time": metadata.state_entered_time.isoformat() if metadata.state_entered_time else None,
            "service_config": metadata.service_config,
            "consecutive_failures": metadata.consecutive_failures,
            "error_message": metadata.error_message,
            "last_ping_time": metadata.last_ping_time.isoformat() if metadata.last_ping_time else None,
        }
        await self._kv_store.put(key=service_name, value=metadata_data, collection=collection)

    async def delete_service_metadata(self, agent_id: str, service_name: str) -> None:
        collection = self._collection(agent_id, "metadata")
        await self._kv_store.delete(key=service_name, collection=collection)

    # Tools ---------------------------------------------------------------

    async def get_tool(self, agent_id: str, tool_name: str) -> Optional[Dict[str, Any]]:
        collection = self._collection(agent_id, "tools")
        value = await self._kv_store.get(key=tool_name, collection=collection)
        return value if isinstance(value, dict) else None

    async def set_tool(self, agent_id: str, tool_name: str, tool_def: Dict[str, Any]) -> None:
        collection = self._collection(agent_id, "tools")
        await self._kv_store.put(key=tool_name, value=tool_def, collection=collection)

    async def delete_tool(self, agent_id: str, tool_name: str) -> None:
        collection = self._collection(agent_id, "tools")
        await self._kv_store.delete(key=tool_name, collection=collection)

    async def list_tools(self, agent_id: str) -> Dict[str, Dict[str, Any]]:
        collection = self._collection(agent_id, "tools")
        if hasattr(self._kv_store, "keys"):
            keys = await self._kv_store.keys(collection=collection)
        else:
            logger.warning(f"Store does not support keys() for collection {collection}")
            return {}
        if not keys:
            return {}
        if hasattr(self._kv_store, "get_many"):
            values = await self._kv_store.get_many(keys, collection=collection)
            return {k: v for k, v in zip(keys, values) if isinstance(v, dict)}
        result: Dict[str, Dict[str, Any]] = {}
        for key in keys:
            value = await self._kv_store.get(key=key, collection=collection)
            if isinstance(value, dict):
                result[key] = value
        return result

    # Tool → service mapping ---------------------------------------------

    async def get_tool_service(self, agent_id: str, tool_name: str) -> Optional[str]:
        collection = self._collection(agent_id, "mappings")
        wrapped = await self._kv_store.get(key=tool_name, collection=collection)
        if wrapped is None:
            return None
        if isinstance(wrapped, dict) and "value" in wrapped:
            return wrapped["value"]
        logger.info("Migrating legacy unwrapped mapping value")
        return wrapped if isinstance(wrapped, str) else None

    async def set_tool_service(self, agent_id: str, tool_name: str, service_name: str) -> None:
        collection = self._collection(agent_id, "mappings")
        wrapped = {"value": service_name}
        await self._kv_store.put(key=tool_name, value=wrapped, collection=collection)

    async def delete_tool_service(self, agent_id: str, tool_name: str) -> None:
        collection = self._collection(agent_id, "mappings")
        await self._kv_store.delete(key=tool_name, collection=collection)

    # Service ↔ client mappings & client configs -------------------------

    async def get_service_client(self, agent_id: str, service_name: str) -> Optional[str]:
        collection = self._collection(agent_id, "mappings")
        key = f"service_client:{service_name}"
        value = await self._kv_store.get(key=key, collection=collection)
        if value is None:
            return None
        if isinstance(value, dict) and "value" in value:
            return value["value"]
        logger.info("Migrating legacy service_client mapping value")
        return value if isinstance(value, str) else None

    async def set_service_client(self, agent_id: str, service_name: str, client_id: str) -> None:
        collection = self._collection(agent_id, "mappings")
        key = f"service_client:{service_name}"
        wrapped = {"value": client_id}
        await self._kv_store.put(key=key, value=wrapped, collection=collection)

    async def delete_service_client(self, agent_id: str, service_name: str) -> None:
        collection = self._collection(agent_id, "mappings")
        key = f"service_client:{service_name}"
        await self._kv_store.delete(key=key, collection=collection)

    async def list_agent_clients(self, agent_id: str) -> List[str]:
        collection = self._collection(agent_id, "mappings")
        if not hasattr(self._kv_store, "keys"):
            return []
        keys = await self._kv_store.keys(collection=collection)
        if not keys:
            return []
        service_keys = [k for k in keys if isinstance(k, str) and k.startswith("service_client:")]
        if not service_keys:
            return []
        values: List[Any]
        if hasattr(self._kv_store, "get_many"):
            values = await self._kv_store.get_many(service_keys, collection=collection)
        else:
            values = []
            for key in service_keys:
                values.append(await self._kv_store.get(key=key, collection=collection))
        client_ids = set()
        for value in values:
            if isinstance(value, dict) and "value" in value:
                client_ids.add(value["value"])
            elif isinstance(value, str):
                client_ids.add(value)
        return list(client_ids)

    async def get_client_config(self, client_id: str) -> Optional[Dict[str, Any]]:
        collection = "clients:configs"
        value = await self._kv_store.get(key=client_id, collection=collection)
        return value if isinstance(value, dict) else None

    async def set_client_config(self, client_id: str, config: Dict[str, Any]) -> None:
        collection = "clients:configs"
        await self._kv_store.put(key=client_id, value=config, collection=collection)

    async def delete_client_config(self, client_id: str) -> None:
        collection = "clients:configs"
        await self._kv_store.delete(key=client_id, collection=collection)
