"""
缓存层管理器

负责管理三层缓存架构的访问和操作：
- 实体层 (Entity Layer)
- 关系层 (Relationship Layer)  
- 状态层 (State Layer)
"""

import asyncio
import copy
import logging
import time
from typing import Any, Dict, Optional, List, TYPE_CHECKING

if TYPE_CHECKING:
    from key_value.aio.protocols import AsyncKeyValue

logger = logging.getLogger(__name__)


class CacheLayerManager:
    """
    缓存层管理器
    
    使用 py-key-value (pyvk) 的 Collection 机制实现三层数据隔离。
    Collection 命名格式: {namespace}:{layer}:{type}
    """
    
    _EMPTY_LOG_INTERVAL_SECONDS = 60.0
    _SCAN_LOG_INTERVAL_SECONDS = 60.0

    def __init__(self, kv_store: 'AsyncKeyValue', namespace: str = "default"):
        """
        初始化缓存层管理器
        
        Args:
            kv_store: pykv 的 AsyncKeyValue 实例
            namespace: 命名空间，默认为 "default"
        """
        self._kv_store = kv_store
        self._namespace = namespace
        # 所有 pykv 调用统一通过 AOB 所属的事件循环执行，避免跨 loop Future 冲突
        try:
            from mcpstore.core.bridge import get_async_bridge  # 延迟导入避免循环依赖
            self._bridge = get_async_bridge()
        except Exception:
            self._bridge = None
        self._last_empty_log: Dict[str, float] = {}
        self._last_scan_log: Dict[str, float] = {}
        self._last_state_snapshot: Dict[str, Any] = {}
        logger.debug(f"[CACHE] [INIT] Initializing CacheLayerManager, namespace: {namespace}")

    async def _await_in_bridge(self, coro, op_name: str):
        """
        确保在 AOB 事件循环中执行 pykv 协程，防止不同事件循环的锁冲突。
        """
        if self._bridge is None:
            return await coro

        bridge_loop = getattr(self._bridge, "_loop", None)
        try:
            running_loop = asyncio.get_running_loop()
        except RuntimeError:
            running_loop = None

        # 已在桥接循环内，直接执行
        if bridge_loop and running_loop is bridge_loop:
            return await coro

        # 无事件循环（同步调用场景）
        if running_loop is None:
            return self._bridge.run(coro, op_name=op_name)

        # 其他事件循环内，切换到 AOB loop
        return await asyncio.to_thread(self._bridge.run, coro, op_name=op_name)
    
    # ==================== Collection 命名方法 ====================
    
    def _get_entity_collection(self, entity_type: str) -> str:
        """
        生成实体层 Collection 名称
        
        格式: {namespace}:entity:{entity_type}
        
        Args:
            entity_type: 实体类型，如 "services", "tools", "agents", "store"
            
        Returns:
            Collection 名称
        """
        return f"{self._namespace}:entity:{entity_type}"
    
    def _get_relation_collection(self, relation_type: str) -> str:
        """
        生成关系层 Collection 名称
        
        格式: {namespace}:relations:{relation_type}
        
        Args:
            relation_type: 关系类型，如 "agent_services", "service_tools"
            
        Returns:
            Collection 名称
        """
        return f"{self._namespace}:relations:{relation_type}"
    
    def _get_state_collection(self, state_type: str) -> str:
        """
        生成状态层 Collection 名称
        
        格式: {namespace}:state:{state_type}
        
        Args:
            state_type: 状态类型，如 "service_status"
            
        Returns:
            Collection 名称
        """
        return f"{self._namespace}:state:{state_type}"

    def _log_empty_collection(self, collection: str):
        """限制空集合调试日志的打印频率，避免刷屏"""
        now = time.time()
        last_logged = self._last_empty_log.get(collection, 0.0)
        if now - last_logged >= self._EMPTY_LOG_INTERVAL_SECONDS:
            logger.debug(f"[CACHE] [EMPTY] Collection is empty: collection={collection}")
            self._last_empty_log[collection] = now

    def _should_log_scan(self, key: str) -> bool:
        """控制扫描日志输出频率"""
        now = time.time()
        last_logged = self._last_scan_log.get(key, 0.0)
        if now - last_logged >= self._SCAN_LOG_INTERVAL_SECONDS:
            self._last_scan_log[key] = now
            return True
        return False

    def _has_state_changed(self, key: str, value: Any) -> bool:
        """检查状态是否发生变化（用于减少重复日志）"""
        sentinel = object()
        previous = self._last_state_snapshot.get(key, sentinel)
        if previous is sentinel:
            self._last_state_snapshot[key] = copy.deepcopy(value)
            return True
        if previous != value:
            self._last_state_snapshot[key] = copy.deepcopy(value)
            return True
        return False
    
    # ==================== 实体层操作 ====================
    
    async def put_entity(
        self, 
        entity_type: str, 
        key: str, 
        value: Dict[str, Any]
    ) -> None:
        """
        存储实体到实体层
        
        Args:
            entity_type: 实体类型
            key: 实体的唯一标识
            value: 实体数据（必须是字典）
            
        Raises:
            ValueError: 如果 value 不是字典类型
            RuntimeError: 如果 pykv 操作失败
        """
        if not isinstance(value, dict):
            raise ValueError(
                f"实体值必须是字典类型，实际类型: {type(value).__name__}. "
                f"entity_type={entity_type}, key={key}"
            )
        
        collection = self._get_entity_collection(entity_type)
        logger.debug(
            f"[CACHE] put_entity: collection={collection}, key={key}, "
            f"entity_type={entity_type}, kv_store instance={id(self._kv_store)}"
        )
        
        try:
            logger.debug(f"[CACHE] [PUT] Calling put: key={key}, collection={collection}, value={value}")
            await self._await_in_bridge(
                self._kv_store.put(key, value, collection=collection),
                f"cache.put_entity.{entity_type}"
            )

            # 调试：检查写入后的内部状态
            if hasattr(self._kv_store, '_cache'):
                cache_keys = list(self._kv_store._cache.keys())
                logger.debug(f"[CACHE] [WRITE] After write, _cache contains {len(cache_keys)} keys: {cache_keys}")
                # 检查具体写入的数据
                if collection in self._kv_store._cache:
                    logger.debug(f"[CACHE] [DATA] Collection {collection} data: {self._kv_store._cache[collection]}")
                else:
                    logger.debug(f"[CACHE] [MISS] Collection {collection} does not exist in _cache")

        except Exception as e:
            logger.error(
                f"[CACHE] [ERROR] Failed to store entity: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"Failed to store entity: collection={collection}, key={key}, error={e}"
            ) from e
    
    async def get_entity(
        self, 
        entity_type: str, 
        key: str
    ) -> Optional[Dict[str, Any]]:
        """
        从实体层获取实体
        
        Args:
            entity_type: 实体类型
            key: 实体的唯一标识
            
        Returns:
            实体数据字典，如果不存在返回 None
            
        Raises:
            RuntimeError: 如果 pykv 操作失败
        """
        if entity_type == "client_configs":
            raise RuntimeError("entity_type 'client_configs' is deprecated, please use 'clients'")

        collection = self._get_entity_collection(entity_type)
        logger.debug(
            f"[CACHE] get_entity: collection={collection}, key={key}, "
            f"entity_type={entity_type}"
        )
        
        try:
            result = await self._await_in_bridge(
                self._kv_store.get(key, collection=collection),
                f"cache.get_entity.{entity_type}"
            )
            return result
        except Exception as e:
            logger.error(
                f"[CACHE] [ERROR] Failed to get entity: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"Failed to get entity: collection={collection}, key={key}, error={e}"
            ) from e
    
    async def delete_entity(self, entity_type: str, key: str) -> None:
        """
        从实体层删除实体
        
        Args:
            entity_type: 实体类型
            key: 实体的唯一标识
            
        Raises:
            RuntimeError: 如果 pykv 操作失败
        """
        collection = self._get_entity_collection(entity_type)
        logger.debug(
            f"[CACHE] delete_entity: collection={collection}, key={key}, "
            f"entity_type={entity_type}"
        )
        
        try:
            await self._await_in_bridge(
                self._kv_store.delete(key, collection=collection),
                f"cache.delete_entity.{entity_type}"
            )
        except Exception as e:
            logger.error(
                f"[CACHE] [ERROR] Failed to delete entity: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"Failed to delete entity: collection={collection}, key={key}, error={e}"
            ) from e
    
    async def get_many_entities(
        self,
        entity_type: str,
        keys: List[str]
    ) -> List[Optional[Dict[str, Any]]]:
        """
        批量获取实体
        
        Args:
            entity_type: 实体类型
            keys: 实体的唯一标识列表
            
        Returns:
            实体数据列表，不存在的实体返回 None
            
        Raises:
            RuntimeError: 如果 pykv 操作失败
        """
        if entity_type == "client_configs":
            raise RuntimeError("entity_type 'client_configs' is deprecated, please use 'clients'")

        collection = self._get_entity_collection(entity_type)
        logger.debug(
            f"[CACHE] get_many_entities: collection={collection}, "
            f"keys_count={len(keys)}, entity_type={entity_type}"
        )
        
        try:
            results = await self._await_in_bridge(
                self._kv_store.get_many(keys, collection=collection),
                f"cache.get_many_entities.{entity_type}"
            )
            return results
        except Exception as e:
            logger.error(
                f"[CACHE] [ERROR] Failed to get many entities: collection={collection}, "
                f"keys_count={len(keys)}, error={e}"
            )
            raise RuntimeError(
                f"Failed to get many entities: collection={collection}, "
                f"keys_count={len(keys)}, error={e}"
            ) from e

    def get_all_entities_sync(self, entity_type: str) -> Dict[str, Dict[str, Any]]:
        """
        同步获取指定类型的所有实体

        这个方法严格遵守核心原则：
        - 通过 pykv 缓存读取，不绕过任何接口
        - 使用同步异步转换在最外层
        - 保持纯计算和IO操作的分离

        Args:
            entity_type: 实体类型

        Returns:
            Dict[str, Dict[str, Any]]: 实体数据字典 {key: entity_data}

        Raises:
            RuntimeError: 如果 pykv 操作失败
        """
        logger.debug(f"[CACHE] get_all_entities_sync: entity_type={entity_type}")

        if entity_type == "client_configs":
            raise RuntimeError("entity_type 'client_configs' is deprecated, please use 'clients'")

        async def _get_all_entities_async():
            """异步内部方法：只使用 await"""
            entities: Dict[str, Dict[str, Any]] = {}
            collection = self._get_entity_collection(entity_type)
            logger.debug(f"[CACHE] [GET] _get_all_entities_async: collection={collection}")

            entity_keys = await self._kv_store.keys(collection=collection)
            logger.debug(f"[CACHE] [GET] Retrieved {len(entity_keys)} keys from collection={collection}")

            if not entity_keys:
                return {}

            results = await self._kv_store.get_many(entity_keys, collection=collection)
            
            for i, key in enumerate(entity_keys):
                if i < len(results) and results[i] is not None:
                    entities[key] = results[i]

            logger.debug(f"[CACHE] [GET] _get_all_entities_async completed: found {len(entities)} entities")
            return entities

        try:
            if self._bridge:
                return self._bridge.run(_get_all_entities_async(), op_name=f"cache.get_all_entities_sync.{entity_type}")
            # 回退：无桥接时使用 asyncio.run（MemoryStore 场景）
            return asyncio.run(_get_all_entities_async())
        except Exception as e:
            logger.error(f"[CACHE] [ERROR] Failed to get all entities synchronously: entity_type={entity_type}, error={e}")
            raise RuntimeError(f"Failed to get all entities synchronously: entity_type={entity_type}, error={e}") from e

    async def get_all_entities_async(self, entity_type: str) -> Dict[str, Dict[str, Any]]:
        """
        异步获取指定类型的所有实体

        遵循核心原则：
        - 只使用 await，不使用 asyncio.run()
        - 在现有事件循环中执行
        - 通过 pykv 接口读取数据
        - 正确传递 collection 参数给 keys() 方法

        Args:
            entity_type: 实体类型

        Returns:
            Dict[str, Dict[str, Any]]: 实体数据字典 {key: entity_data}
            
        Raises:
            RuntimeError: 如果 pykv 操作失败
        """
        if entity_type == "client_configs":
            raise RuntimeError("entity_type 'client_configs' is deprecated, please use 'clients'")

        log_key = f"entity_scan:{entity_type}"
        log_scan = self._should_log_scan(log_key)
        if log_scan:
            logger.debug(f"[CACHE] get_all_entities_async: entity_type={entity_type}")

        async def _read():
            collection = self._get_entity_collection(entity_type)
            if log_scan:
                logger.debug(f"[CACHE] get_all_entities_async: collection={collection}, entity_type={entity_type}")

            entity_keys = await self._kv_store.keys(collection=collection)

            if log_scan:
                logger.debug(f"[CACHE] [GET] Retrieved {len(entity_keys)} keys from collection={collection}")

            if not entity_keys:
                self._log_empty_collection(collection)
                return {}

            # 批量获取实体数据
            results = await self._kv_store.get_many(entity_keys, collection=collection)
            
            entities: Dict[str, Dict[str, Any]] = {}
            for i, key in enumerate(entity_keys):
                if i < len(results) and results[i] is not None:
                    entities[key] = results[i]

            if log_scan:
                logger.debug(f"[CACHE] [GET] get_all_entities_async completed: found {len(entities)} entities")

            return entities

        try:
            return await self._await_in_bridge(_read(), f"cache.get_all_entities_async.{entity_type}")
        except Exception as e:
            logger.error(f"[CACHE] [ERROR] Failed to get all entities asynchronously: entity_type={entity_type}, error={e}")
            raise RuntimeError(f"Failed to get all entities asynchronously: entity_type={entity_type}, error={e}") from e

    # ==================== 关系层操作 ====================
    
    async def put_relation(
        self,
        relation_type: str,
        key: str,
        value: Dict[str, Any]
    ) -> None:
        """
        存储关系到关系层
        
        Args:
            relation_type: 关系类型
            key: 关系的唯一标识
            value: 关系数据（必须是字典）
            
        Raises:
            ValueError: 如果 value 不是字典类型
            RuntimeError: 如果 pykv 操作失败
        """
        if not isinstance(value, dict):
            raise ValueError(
                f"关系值必须是字典类型，实际类型: {type(value).__name__}. "
                f"relation_type={relation_type}, key={key}"
            )
        
        collection = self._get_relation_collection(relation_type)
        logger.debug(
            f"[CACHE] put_relation: collection={collection}, key={key}, "
            f"relation_type={relation_type}"
        )
        
        try:
            await self._await_in_bridge(
                self._kv_store.put(key, value, collection=collection),
                f"cache.put_relation.{relation_type}"
            )
        except Exception as e:
            logger.error(
                f"[CACHE] [ERROR] Failed to store relation: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"存储关系失败: collection={collection}, key={key}, error={e}"
            ) from e
    
    async def get_relation(
        self,
        relation_type: str,
        key: str
    ) -> Optional[Dict[str, Any]]:
        """
        从关系层获取关系
        
        Args:
            relation_type: 关系类型
            key: 关系的唯一标识
            
        Returns:
            关系数据字典，如果不存在返回 None
            
        Raises:
            RuntimeError: 如果 pykv 操作失败
        """
        collection = self._get_relation_collection(relation_type)
        logger.debug(
            f"[CACHE] get_relation: collection={collection}, key={key}, "
            f"relation_type={relation_type}"
        )
        
        try:
            result = await self._await_in_bridge(
                self._kv_store.get(key, collection=collection),
                f"cache.get_relation.{relation_type}"
            )
            return result
        except Exception as e:
            logger.error(
                f"[CACHE] [ERROR] Failed to get relation: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"获取关系失败: collection={collection}, key={key}, error={e}"
            ) from e
    
    async def delete_relation(self, relation_type: str, key: str) -> None:
        """
        从关系层删除关系
        
        Args:
            relation_type: 关系类型
            key: 关系的唯一标识
            
        Raises:
            RuntimeError: 如果 pykv 操作失败
        """
        collection = self._get_relation_collection(relation_type)
        logger.debug(
            f"[CACHE] delete_relation: collection={collection}, key={key}, "
            f"relation_type={relation_type}"
        )
        
        try:
            await self._await_in_bridge(
                self._kv_store.delete(key, collection=collection),
                f"cache.delete_relation.{relation_type}"
            )
        except Exception as e:
            logger.error(
                f"[CACHE] [ERROR] Failed to delete relation: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"删除关系失败: collection={collection}, key={key}, error={e}"
            ) from e

    async def get_all_relations_async(self, relation_type: str) -> Dict[str, Dict[str, Any]]:
        """
        异步获取指定类型的所有关系
        """
        collection = self._get_relation_collection(relation_type)
        logger.debug(f"[CACHE] get_all_relations_async: collection={collection}, relation_type={relation_type}")
        log_key = f"relation_scan:{relation_type}"
        log_scan = self._should_log_scan(log_key)
        async def _read():
            relation_keys = await self._kv_store.keys(collection=collection)
            if not relation_keys:
                self._log_empty_collection(collection)
                return {}

            results = await self._kv_store.get_many(relation_keys, collection=collection)
            relations: Dict[str, Dict[str, Any]] = {}
            for i, key in enumerate(relation_keys):
                if i < len(results) and results[i] is not None:
                    relations[key] = results[i]

            if log_scan:
                logger.debug(f"[CACHE] [GET] get_all_relations_async completed: found {len(relations)} relations")
            return relations

        try:
            return await self._await_in_bridge(_read(), f"cache.get_all_relations_async.{relation_type}")
        except Exception as e:
            logger.error(f"[CACHE] [ERROR] Failed to get all relations asynchronously: relation_type={relation_type}, error={e}")
            raise RuntimeError(f"Failed to get all relations asynchronously: relation_type={relation_type}, error={e}") from e
    
    # ==================== 状态层操作 ====================
    
    async def put_state(
        self,
        state_type: str,
        key: str,
        value: Dict[str, Any]
    ) -> None:
        """
        存储状态到状态层
        
        Args:
            state_type: 状态类型
            key: 状态的唯一标识
            value: 状态数据（必须是字典）
            
        Raises:
            ValueError: 如果 value 不是字典类型
            RuntimeError: 如果 pykv 操作失败
        """
        if not isinstance(value, dict):
            raise ValueError(
                f"状态值必须是字典类型，实际类型: {type(value).__name__}. "
                f"state_type={state_type}, key={key}"
            )
        
        collection = self._get_state_collection(state_type)
        logger.debug(
            f"[CACHE] put_state: collection={collection}, key={key}, "
            f"state_type={state_type}"
        )
        try:
            logger.info(f"[CACHE] [STATE] Storing state value: collection={collection}, key={key}, value={value}")
            await self._await_in_bridge(
                self._kv_store.put(key, value, collection=collection),
                f"cache.put_state.{state_type}"
            )
            logger.info(f"[CACHE] [STATE] State stored successfully: collection={collection}, key={key}")
        except Exception as e:
            logger.error(
                f"[CACHE] [ERROR] Failed to store state: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"存储状态失败: collection={collection}, key={key}, error={e}"
            ) from e
    
    async def get_state(
        self,
        state_type: str,
        key: str
    ) -> Optional[Dict[str, Any]]:
        """
        从状态层获取状态
        
        Args:
            state_type: 状态类型
            key: 状态的唯一标识
            
        Returns:
            状态数据字典，如果不存在返回 None
            
        Raises:
            RuntimeError: 如果 pykv 操作失败
        """
        collection = self._get_state_collection(state_type)
        state_key = f"{state_type}:{key}"
        log_key = f"state_read:{state_key}"
        log_state = self._should_log_scan(log_key)
        if log_state:
            logger.debug(
                f"[CACHE] get_state: collection={collection}, key={key}, "
                f"state_type={state_type}"
            )

        try:
            result = await self._await_in_bridge(
                self._kv_store.get(key, collection=collection),
                f"cache.get_state.{state_type}"
            )
            if log_state or self._has_state_changed(state_key, result):
                logger.debug(f"[CACHE] [STATE] Reading state value: collection={collection}, key={key}, result={result}")
            return result
        except Exception as e:
            logger.error(
                f"[CACHE] [ERROR] Failed to get state: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"获取状态失败: collection={collection}, key={key}, error={e}"
            ) from e

    async def get_all_states_async(self, state_type: str) -> Dict[str, Dict[str, Any]]:
        """
        异步获取指定类型的所有状态
        """
        collection = self._get_state_collection(state_type)
        logger.debug(f"[CACHE] get_all_states_async: collection={collection}, state_type={state_type}")
        log_key = f"state_scan:{state_type}"
        log_scan = self._should_log_scan(log_key)
        async def _read():
            state_keys = await self._kv_store.keys(collection=collection)
            if not state_keys:
                self._log_empty_collection(collection)
                return {}

            results = await self._kv_store.get_many(state_keys, collection=collection)
            states: Dict[str, Dict[str, Any]] = {}
            for i, key in enumerate(state_keys):
                if i < len(results) and results[i] is not None:
                    states[key] = results[i]

            if log_scan:
                logger.debug(f"[CACHE] [GET] get_all_states_async completed: found {len(states)} states")
            return states

        try:
            return await self._await_in_bridge(_read(), f"cache.get_all_states_async.{state_type}")
        except Exception as e:
            logger.error(f"[CACHE] [ERROR] Failed to get all states asynchronously: state_type={state_type}, error={e}")
            raise RuntimeError(f"Failed to get all states asynchronously: state_type={state_type}, error={e}") from e
    
    async def delete_state(self, state_type: str, key: str) -> None:
        """
        从状态层删除状态
        
        Args:
            state_type: 状态类型
            key: 状态的唯一标识
            
        Raises:
            RuntimeError: 如果 pykv 操作失败
        """
        collection = self._get_state_collection(state_type)
        logger.debug(
            f"[CACHE] delete_state: collection={collection}, key={key}, "
            f"state_type={state_type}"
        )
        
        try:
            await self._await_in_bridge(
                self._kv_store.delete(key, collection=collection),
                f"cache.delete_state.{state_type}"
            )
        except Exception as e:
            logger.error(
                f"[CACHE] [ERROR] Failed to delete state: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"删除状态失败: collection={collection}, key={key}, error={e}"
            ) from e

    def put_state_sync(
        self,
        state_type: str,
        key: str,
        value: Dict[str, Any]
    ) -> None:
        """
        同步存储状态到状态层
        
        遵循核心原则：同步外壳在最外层使用一次 asyncio.run()
        
        Args:
            state_type: 状态类型
            key: 状态的唯一标识
            value: 状态数据（必须是字典）
            
        Raises:
            ValueError: 如果 value 不是字典类型
            RuntimeError: 如果 pykv 操作失败
        """
        import asyncio
        
        if not isinstance(value, dict):
            raise ValueError(
                f"状态值必须是字典类型，实际类型: {type(value).__name__}. "
                f"state_type={state_type}, key={key}"
            )
        
        collection = self._get_state_collection(state_type)
        logger.debug(
            f"[CACHE] put_state_sync: collection={collection}, key={key}, "
            f"state_type={state_type}"
        )

        async def _put_state_async():
            """异步内部方法：只使用 await"""
            await self._kv_store.put(key, value, collection=collection)

        try:
            if self._bridge:
                self._bridge.run(_put_state_async(), op_name=f"cache.put_state_sync.{state_type}")
            else:
                asyncio.run(_put_state_async())
            
            logger.info(f"[CACHE] [STATE] Synchronous state storage successful: collection={collection}, key={key}")
        except Exception as e:
            logger.error(
                f"[CACHE] [ERROR] Failed to store state synchronously: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"同步存储状态失败: collection={collection}, key={key}, error={e}"
            ) from e

    def get_state_sync(
        self,
        state_type: str,
        key: str
    ) -> Optional[Dict[str, Any]]:
        """
        同步从状态层获取状态
        
        遵循核心原则：同步外壳在最外层使用一次 asyncio.run()
        
        Args:
            state_type: 状态类型
            key: 状态的唯一标识
            
        Returns:
            状态数据字典，如果不存在返回 None
            
        Raises:
            RuntimeError: 如果 pykv 操作失败
        """
        import asyncio
        
        collection = self._get_state_collection(state_type)
        logger.debug(
            f"[CACHE] get_state_sync: collection={collection}, key={key}, "
            f"state_type={state_type}"
        )

        async def _get_state_async():
            """异步内部方法：只使用 await"""
            return await self._kv_store.get(key, collection=collection)

        try:
            if self._bridge:
                result = self._bridge.run(_get_state_async(), op_name=f"cache.get_state_sync.{state_type}")
            else:
                result = asyncio.run(_get_state_async())
            
            logger.debug(f"[CACHE] [GET] Synchronous state retrieval successful: collection={collection}, key={key}")
            return result
        except Exception as e:
            logger.error(
                f"[CACHE] [ERROR] Failed to get state synchronously: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"同步获取状态失败: collection={collection}, key={key}, error={e}"
            ) from e
    
    # ==================== Agent 实体操作 ====================
    
    async def create_agent(
        self,
        agent_id: str,
        created_time: int,
        is_global: bool = False
    ) -> None:
        """
        创建 Agent 实体
        
        Args:
            agent_id: Agent ID
            created_time: 创建时间戳
            is_global: 是否为全局代理
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果创建失败
        """
        if not agent_id:
            raise ValueError("Agent ID cannot be empty")
        
        from .models import AgentEntity
        
        # 检查 Agent 是否已存在
        existing = await self.get_entity("agents", agent_id)
        if existing:
            raise ValueError(f"Agent already exists: agent_id={agent_id}")
        
        # 创建 Agent 实体
        entity = AgentEntity(
            agent_id=agent_id,
            created_time=created_time,
            last_active=created_time,
            is_global=is_global
        )
        
        # 存储到实体层
        await self.put_entity("agents", agent_id, entity.to_dict())
        
        logger.info(
            f"[CACHE] [AGENT] Created Agent entity: agent_id={agent_id}, "
            f"is_global={is_global}"
        )
    
    async def get_agent(self, agent_id: str) -> Optional[Dict[str, Any]]:
        """
        获取 Agent 实体
        
        Args:
            agent_id: Agent ID
            
        Returns:
            Agent 实体数据，如果不存在返回 None
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果获取失败
        """
        if not agent_id:
            raise ValueError("Agent ID cannot be empty")
        
        # 从实体层获取
        data = await self.get_entity("agents", agent_id)
        
        if data is None:
            logger.debug(f"[CACHE] [AGENT] Agent does not exist: agent_id={agent_id}")
            return None
        
        logger.debug(f"[CACHE] [AGENT] Getting Agent entity: agent_id={agent_id}")
        return data
    
    async def update_agent_last_active(
        self,
        agent_id: str,
        last_active: int
    ) -> None:
        """
        更新 Agent 最后活跃时间
        
        Args:
            agent_id: Agent ID
            last_active: 最后活跃时间戳
            
        Raises:
            ValueError: 如果参数无效
            KeyError: 如果 Agent 不存在
            RuntimeError: 如果更新失败
        """
        if not agent_id:
            raise ValueError("Agent ID cannot be empty")
        
        # 获取现有 Agent
        data = await self.get_agent(agent_id)
        if data is None:
            raise KeyError(f"Agent does not exist: agent_id={agent_id}")
        
        # 更新最后活跃时间
        data["last_active"] = last_active
        
        # 保存到实体层
        await self.put_entity("agents", agent_id, data)
        
        logger.debug(
            f"[CACHE] [AGENT] Updating Agent last active time: agent_id={agent_id}, "
            f"last_active={last_active}"
        )
    
    # ==================== Store 配置操作 ====================
    
    async def set_store_config(self, config: Dict[str, Any]) -> None:
        """
        设置 Store 配置
        
        Args:
            config: Store 配置数据
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果设置失败
        """
        if not isinstance(config, dict):
            raise ValueError(
                f"Store 配置必须是字典类型，实际类型: {type(config).__name__}"
            )
        
        from .models import StoreConfig
        
        # 验证配置数据
        try:
            StoreConfig.from_dict(config)
        except Exception as e:
            raise ValueError(f"Invalid Store configuration: {e}") from e
        
        # 存储到实体层，使用固定的 key "mcpstore"
        await self.put_entity("store", "mcpstore", config)
        
        logger.info("[CACHE] [STORE] Setting Store configuration")
    
    async def get_store_config(self) -> Optional[Dict[str, Any]]:
        """
        获取 Store 配置
        
        Returns:
            Store 配置数据，如果不存在返回 None
            
        Raises:
            RuntimeError: 如果获取失败
        """
        # 从实体层获取，使用固定的 key "mcpstore"
        data = await self.get_entity("store", "mcpstore")
        
        if data is None:
            logger.debug("[CACHE] [STORE] Store configuration does not exist")
            return None
        
        logger.debug("[CACHE] [STORE] Getting Store configuration")
        return data
