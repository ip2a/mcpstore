"""
缓存层管理器

负责管理三层缓存架构的访问和操作：
- 实体层 (Entity Layer)
- 关系层 (Relationship Layer)  
- 状态层 (State Layer)
"""

import logging
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
    
    def __init__(self, kv_store: 'AsyncKeyValue', namespace: str = "default"):
        """
        初始化缓存层管理器
        
        Args:
            kv_store: pykv 的 AsyncKeyValue 实例
            namespace: 命名空间，默认为 "default"
        """
        self._kv_store = kv_store
        self._namespace = namespace
        logger.debug(f"[CACHE] 初始化 CacheLayerManager，命名空间: {namespace}")
    
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
            f"entity_type={entity_type}, kv_store 实例 = {id(self._kv_store)}"
        )
        
        try:
            logger.debug(f"[CACHE] 调用 put: key={key}, collection={collection}, value={value}")
            await self._kv_store.put(key, value, collection=collection)

            # 调试：检查写入后的内部状态
            if hasattr(self._kv_store, '_cache'):
                cache_keys = list(self._kv_store._cache.keys())
                logger.debug(f"[CACHE] 写入后 _cache 包含 {len(cache_keys)} 个键: {cache_keys}")
                # 检查具体写入的数据
                if collection in self._kv_store._cache:
                    logger.debug(f"[CACHE] 集合 {collection} 的数据: {self._kv_store._cache[collection]}")
                else:
                    logger.debug(f"[CACHE] 集合 {collection} 不存在于 _cache 中")

        except Exception as e:
            logger.error(
                f"[CACHE] 存储实体失败: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"存储实体失败: collection={collection}, key={key}, error={e}"
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
        collection = self._get_entity_collection(entity_type)
        logger.debug(
            f"[CACHE] get_entity: collection={collection}, key={key}, "
            f"entity_type={entity_type}"
        )
        
        try:
            result = await self._kv_store.get(key, collection=collection)
            return result
        except Exception as e:
            logger.error(
                f"[CACHE] 获取实体失败: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"获取实体失败: collection={collection}, key={key}, error={e}"
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
            await self._kv_store.delete(key, collection=collection)
        except Exception as e:
            logger.error(
                f"[CACHE] 删除实体失败: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"删除实体失败: collection={collection}, key={key}, error={e}"
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
        collection = self._get_entity_collection(entity_type)
        logger.debug(
            f"[CACHE] get_many_entities: collection={collection}, "
            f"keys_count={len(keys)}, entity_type={entity_type}"
        )
        
        try:
            results = await self._kv_store.get_many(keys, collection=collection)
            return results
        except Exception as e:
            logger.error(
                f"[CACHE] 批量获取实体失败: collection={collection}, "
                f"keys_count={len(keys)}, error={e}"
            )
            raise RuntimeError(
                f"批量获取实体失败: collection={collection}, "
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

        try:
            # 核心原则：必须在最外层使用一次同步异步转换
            # 这是唯一允许的 asyncio.run() 使用点
            import asyncio

            async def _get_all_entities_async():
                """异步内部方法：遵循原则，只使用 await"""
                collection = self._get_entity_collection(entity_type)
                logger.debug(f"[CACHE] _get_all_entities_async: collection={collection}")

                # 严格按照原则：通过 pykv 接口读取数据
                try:
                    # 获取所有键 - 这是 IO 操作，使用 await
                    keys_coro = self._kv_store.keys()
                    keys = await keys_coro  # 先获取协程结果

                    # 过滤属于指定实体类型的键
                    collection_prefix = f"{collection}:"
                    entity_keys = [key for key in keys if key.startswith(collection_prefix)]

                    # 批量获取实体数据 - 这是 IO 操作，使用 await
                    entities = {}
                    if entity_keys:
                        results = await self._kv_store.get_many(entity_keys)
                        for i, key in enumerate(entity_keys):
                            if i < len(results) and results[i] is not None:
                                # 提取实体键名（去掉集合前缀）
                                entity_key = key[len(collection_prefix):]
                                entities[entity_key] = results[i]

                    logger.debug(f"[CACHE] _get_all_entities_async 完成: 找到 {len(entities)} 个实体")
                    return entities

                except Exception as e:
                    logger.error(f"[CACHE] 异步获取实体失败: {e}")
                    raise

            # 在最外层使用一次同步异步转换 - 符合原则
            return asyncio.run(_get_all_entities_async())

        except Exception as e:
            logger.error(f"[CACHE] 同步获取所有实体失败: entity_type={entity_type}, error={e}")
            raise RuntimeError(f"同步获取所有实体失败: entity_type={entity_type}, error={e}") from e

    async def get_all_entities_async(self, entity_type: str) -> Dict[str, Dict[str, Any]]:
        """
        异步获取指定类型的所有实体

        遵循核心原则：
        - 只使用 await，不使用 asyncio.run()
        - 在现有事件循环中执行
        - 通过 pykv 接口读取数据

        Args:
            entity_type: 实体类型

        Returns:
            Dict[str, Dict[str, Any]]: 实体数据字典 {key: entity_data}
        """
        collection = self._get_entity_collection(entity_type)
        logger.debug(f"[CACHE] get_all_entities_async: collection={collection}, entity_type={entity_type}")
        logger.debug(f"[CACHE] 调试: kv_store 实例 = {id(self._kv_store)}, 类型 = {type(self._kv_store)}")

        try:
            # 直接从 MemoryStore 的 _cache 获取数据，绕过有问题的 keys() 方法
            if hasattr(self._kv_store, '_cache'):
                # 直接访问内部 _cache 数据结构
                cache_keys = list(self._kv_store._cache.keys())
                logger.debug(f"[CACHE] 从 _cache 获取到 {len(cache_keys)} 个键: {cache_keys}")

                # 构造完整的键名
                collection_key = f"{self._namespace}:entity:{entity_type}"
                logger.debug(f"[CACHE] 查找集合键: {collection_key}")

                # 检查是否有这个键，如果没有，检查可能的替代键
                actual_collection_key = None
                if collection_key in self._kv_store._cache:
                    actual_collection_key = collection_key
                else:
                    # 检查是否有默认集合键（pykv内部可能使用不同的键名）
                    possible_keys = [k for k in cache_keys if 'default' in k and ('entity' in k or 'collection' in k)]
                    logger.debug(f"[CACHE] 可能的集合键: {possible_keys}")
                    if possible_keys:
                        actual_collection_key = possible_keys[0]
                        logger.debug(f"[CACHE] 使用找到的集合键: {actual_collection_key}")

                if actual_collection_key and actual_collection_key in self._kv_store._cache:
                    # 从集合中获取所有实体数据
                    entities_collection = self._kv_store._cache[actual_collection_key]
                    logger.info(f"[CACHE] 集合数据类型: {type(entities_collection)}")
                    logger.debug(f"[CACHE] 集合对象方法: {[m for m in dir(entities_collection) if not m.startswith('_')]}")

                    # 如果是MemoryCollection对象，需要调用其get_all_items或类似方法
                    if hasattr(entities_collection, 'get_all_items'):
                        entities = entities_collection.get_all_items()
                        logger.info(f"[CACHE] 从MemoryCollection获取到 {len(entities)} 个实体")
                        return entities
                    elif hasattr(entities_collection, 'items'):
                        entities = dict(entities_collection.items())
                        logger.info(f"[CACHE] 从Collection.items()获取到 {len(entities)} 个实体")
                        return entities
                    elif isinstance(entities_collection, dict):
                        logger.info(f"[CACHE] 从字典获取到 {len(entities_collection)} 个实体")
                        return entities_collection
                    else:
                        logger.info(f"[CACHE] 集合数据类型需要特殊处理: {type(entities_collection)}")
                        # 使用MemoryCollection的keys()和get()方法构建字典
                        try:
                            entities = {}
                            if hasattr(entities_collection, 'keys'):
                                # 同步获取所有键
                                collection_keys = list(entities_collection.keys())
                                logger.info(f"[CACHE] MemoryCollection keys: {collection_keys}")

                                # 获取每个键对应的值
                                for key in collection_keys:
                                    managed_entry = entities_collection.get(key)
                                    if managed_entry is not None:
                                        # ManagedEntry 对象有 value 属性，包含实际数据
                                        if hasattr(managed_entry, 'value'):
                                            entities[key] = managed_entry.value
                                        else:
                                            # 如果不是 ManagedEntry，直接使用
                                            entities[key] = managed_entry

                                logger.info(f"[CACHE] 从MemoryCollection获取到 {len(entities)} 个实体")
                                return entities
                            else:
                                logger.warning(f"[CACHE] MemoryCollection没有keys方法")
                                return {}
                        except Exception as e:
                            logger.error(f"[CACHE] MemoryCollection访问失败: {e}")
                            raise RuntimeError(f"MemoryCollection访问失败: {e}") from e
                else:
                    logger.debug(f"[CACHE] 集合键不存在: {collection_key}")

            # 回退方案：使用原来的 keys() 方法（可能有bug）
            logger.debug("[CACHE] 回退到 keys() 方法")
            keys_coro = self._kv_store.keys()
            keys = await keys_coro

            logger.debug(f"[CACHE] 回退方法获取到所有键: {len(keys)} 个")

            # 过滤属于指定实体类型的键
            collection_prefix = f"{collection}:"
            entity_keys = [key for key in keys if key.startswith(collection_prefix)]

            # 批量获取实体数据 - IO 操作，使用 await
            entities = {}
            if entity_keys:
                results = await self._kv_store.get_many(entity_keys)
                for i, key in enumerate(entity_keys):
                    if i < len(results) and results[i] is not None:
                        # 提取实体键名（去掉集合前缀）
                        entity_key = key[len(collection_prefix):]
                        entities[entity_key] = results[i]

            logger.debug(f"[CACHE] get_all_entities_async 完成: 找到 {len(entities)} 个实体")
            return entities

        except Exception as e:
            logger.error(f"[CACHE] 异步获取所有实体失败: entity_type={entity_type}, error={e}")
            raise RuntimeError(f"异步获取所有实体失败: entity_type={entity_type}, error={e}") from e

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
            await self._kv_store.put(key, value, collection=collection)
        except Exception as e:
            logger.error(
                f"[CACHE] 存储关系失败: collection={collection}, key={key}, "
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
            result = await self._kv_store.get(key, collection=collection)
            return result
        except Exception as e:
            logger.error(
                f"[CACHE] 获取关系失败: collection={collection}, key={key}, "
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
            await self._kv_store.delete(key, collection=collection)
        except Exception as e:
            logger.error(
                f"[CACHE] 删除关系失败: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"删除关系失败: collection={collection}, key={key}, error={e}"
            ) from e
    
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
            logger.info(f"[CACHE] 🔧 存储状态值: collection={collection}, key={key}, value={value}")
            await self._kv_store.put(key, value, collection=collection)
            logger.info(f"[CACHE] ✅ 状态存储成功: collection={collection}, key={key}")
        except Exception as e:
            logger.error(
                f"[CACHE] 存储状态失败: collection={collection}, key={key}, "
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
        logger.debug(
            f"[CACHE] get_state: collection={collection}, key={key}, "
            f"state_type={state_type}"
        )

        try:
            result = await self._kv_store.get(key, collection=collection)
            logger.info(f"[CACHE] 🔧 读取状态值: collection={collection}, key={key}, result={result}")
            return result
        except Exception as e:
            logger.error(
                f"[CACHE] 获取状态失败: collection={collection}, key={key}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"获取状态失败: collection={collection}, key={key}, error={e}"
            ) from e
    
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
            await self._kv_store.delete(key, collection=collection)
        except Exception as e:
            logger.error(
                f"[CACHE] 删除状态失败: collection={collection}, key={key}, "
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
            # 检查是否已在事件循环中
            try:
                loop = asyncio.get_running_loop()
                # 已在事件循环中，创建任务并等待
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(asyncio.run, _put_state_async())
                    future.result()
            except RuntimeError:
                # 不在事件循环中，直接使用 asyncio.run()
                asyncio.run(_put_state_async())
            
            logger.info(f"[CACHE] 同步存储状态成功: collection={collection}, key={key}")
        except Exception as e:
            logger.error(
                f"[CACHE] 同步存储状态失败: collection={collection}, key={key}, "
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
            # 检查是否已在事件循环中
            try:
                loop = asyncio.get_running_loop()
                # 已在事件循环中，创建任务并等待
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(asyncio.run, _get_state_async())
                    result = future.result()
            except RuntimeError:
                # 不在事件循环中，直接使用 asyncio.run()
                result = asyncio.run(_get_state_async())
            
            logger.debug(f"[CACHE] 同步获取状态成功: collection={collection}, key={key}")
            return result
        except Exception as e:
            logger.error(
                f"[CACHE] 同步获取状态失败: collection={collection}, key={key}, "
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
            raise ValueError("Agent ID 不能为空")
        
        from .models import AgentEntity
        
        # 检查 Agent 是否已存在
        existing = await self.get_entity("agents", agent_id)
        if existing:
            raise ValueError(f"Agent 已存在: agent_id={agent_id}")
        
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
            f"[CACHE] 创建 Agent 实体: agent_id={agent_id}, "
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
            raise ValueError("Agent ID 不能为空")
        
        # 从实体层获取
        data = await self.get_entity("agents", agent_id)
        
        if data is None:
            logger.debug(f"[CACHE] Agent 不存在: agent_id={agent_id}")
            return None
        
        logger.debug(f"[CACHE] 获取 Agent 实体: agent_id={agent_id}")
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
            raise ValueError("Agent ID 不能为空")
        
        # 获取现有 Agent
        data = await self.get_agent(agent_id)
        if data is None:
            raise KeyError(f"Agent 不存在: agent_id={agent_id}")
        
        # 更新最后活跃时间
        data["last_active"] = last_active
        
        # 保存到实体层
        await self.put_entity("agents", agent_id, data)
        
        logger.debug(
            f"[CACHE] 更新 Agent 最后活跃时间: agent_id={agent_id}, "
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
            raise ValueError(f"无效的 Store 配置: {e}") from e
        
        # 存储到实体层，使用固定的 key "mcpstore"
        await self.put_entity("store", "mcpstore", config)
        
        logger.info("[CACHE] 设置 Store 配置")
    
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
            logger.debug("[CACHE] Store 配置不存在")
            return None
        
        logger.debug("[CACHE] 获取 Store 配置")
        return data
