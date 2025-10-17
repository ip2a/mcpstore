"""Redis atomic operations using Lua scripts for data consistency."""
from __future__ import annotations

import json
import logging
from typing import Dict, Any, Optional

logger = logging.getLogger(__name__)


class RedisAtomicOps:
    """Provides atomic operations on Redis using Lua scripts.

    This ensures data consistency for Read-Modify-Write operations
    that would otherwise be subject to race conditions.
    """

    # Lua script: Atomic JSON update (Read-Modify-Write)
    LUA_UPDATE_JSON = """
    local key = KEYS[1]
    local updates_json = ARGV[1]
    local updates = cjson.decode(updates_json)

    -- Read current value
    local current_json = redis.call('GET', key)
    local config = {}
    if current_json then
        config = cjson.decode(current_json)
    end

    -- Merge updates
    for k, v in pairs(updates) do
        config[k] = v
    end

    -- Write back atomically
    redis.call('SET', key, cjson.encode(config))
    return 1
    """

    def __init__(self, redis_client):
        """Initialize with a Redis client.

        Args:
            redis_client: redis.Redis instance
        """
        self._redis = redis_client
        # Pre-load scripts for better performance
        try:
            self._update_json_sha = self._redis.script_load(self.LUA_UPDATE_JSON)
            logger.debug("Redis Lua scripts loaded successfully")
        except Exception as e:
            logger.warning(f"Failed to preload Lua scripts: {e}. Will load on demand.")
            self._update_json_sha = None

    def update_json_atomic(self, key: str, updates: Dict[str, Any]) -> bool:
        """Atomically update a JSON object stored in Redis.

        This performs a Read-Modify-Write operation atomically:
        1. Read current JSON
        2. Merge with updates
        3. Write back

        Args:
            key: Redis key
            updates: Dictionary of updates to merge

        Returns:
            True if successful

        Example:
            >>> ops.update_json_atomic("config:123", {"timeout": 30})
        """
        try:
            updates_json = json.dumps(updates, ensure_ascii=False)

            # Try using preloaded script
            if self._update_json_sha:
                try:
                    self._redis.evalsha(self._update_json_sha, 1, key, updates_json)
                    return True
                except Exception:
                    # Script not found, reload
                    logger.debug("Reloading Lua script (SHA not found)")

            # Fallback: load and execute
            self._redis.eval(self.LUA_UPDATE_JSON, 1, key, updates_json)
            return True

        except Exception as e:
            logger.error(f"Atomic JSON update failed for key {key}: {e}")
            return False
