# Redis 数据库支持测试模块

本模块包含 Redis 数据库支持相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_redis_local.py` | Redis 本地服务支持 | Store 级别 |
| `test_store_redis_remote.py` | Redis 远程服务支持 | Store 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# Redis 本地服务支持
python example/database/redis/test_store_redis_local.py

# Redis 远程服务支持
python example/database/redis/test_store_redis_remote.py
```

### 运行所有 Redis 测试

```bash
# Windows
for %f in (example\database\redis\test_*.py) do python %f

# Linux/Mac
for f in example/database/redis/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. Redis 本地服务支持
测试本地 Redis 服务器支持：
- 本地 Redis 配置
- 数据持久化存储
- 服务管理
- 工具调用
- 性能测试

### 2. Redis 远程服务支持
测试远程 Redis 服务器支持：
- 远程 Redis 配置
- 网络连接管理
- 安全认证
- 数据同步
- 连接稳定性

## 💡 核心概念

### Redis 配置

| 配置项 | 说明 | 本地示例 | 远程示例 |
|--------|------|----------|----------|
| `host` | Redis 服务器地址 | `localhost` | `redis.example.com` |
| `port` | Redis 端口 | `6379` | `6379` |
| `db` | 数据库编号 | `0` | `0` |
| `password` | 认证密码 | `None` | `your_password` |
| `ssl` | SSL 加密 | `False` | `True` |
| `timeout` | 连接超时 | `30` | `30` |

### Redis 特性

| 特性 | 本地服务 | 远程服务 | 用途 |
|------|----------|----------|------|
| **数据持久化** | ✅ | ✅ | 数据保存 |
| **高性能** | ✅ | ✅ | 快速访问 |
| **分布式** | ❌ | ✅ | 多节点 |
| **安全认证** | ❌ | ✅ | 访问控制 |
| **SSL 加密** | ❌ | ✅ | 数据传输 |

## 🎯 使用场景

### 场景 1：本地开发环境
```python
# 本地 Redis 配置
def setup_local_redis():
    redis_config = {
        "redis": {
            "host": "localhost",
            "port": 6379,
            "db": 0,
            "password": None
        }
    }
    
    store = MCPStore.setup_store(debug=True, **redis_config)
    return store
```

### 场景 2：生产环境
```python
# 生产环境 Redis 配置
def setup_production_redis():
    redis_config = {
        "redis": {
            "host": "redis.production.com",
            "port": 6379,
            "db": 0,
            "password": "secure_password",
            "ssl": True,
            "timeout": 30
        }
    }
    
    store = MCPStore.setup_store(debug=False, **redis_config)
    return store
```

### 场景 3：Redis 集群
```python
# Redis 集群配置
def setup_redis_cluster():
    redis_config = {
        "redis": {
            "host": "redis-cluster.example.com",
            "port": 6379,
            "db": 0,
            "password": "cluster_password",
            "ssl": True,
            "timeout": 30,
            "cluster": True
        }
    }
    
    store = MCPStore.setup_store(debug=False, **redis_config)
    return store
```

### 场景 4：Redis 哨兵模式
```python
# Redis 哨兵模式配置
def setup_redis_sentinel():
    redis_config = {
        "redis": {
            "host": "redis-sentinel.example.com",
            "port": 26379,
            "db": 0,
            "password": "sentinel_password",
            "ssl": True,
            "timeout": 30,
            "sentinel": True,
            "master_name": "mymaster"
        }
    }
    
    store = MCPStore.setup_store(debug=False, **redis_config)
    return store
```

## 📊 配置对比

### 本地 vs 远程 Redis

| 方面 | 本地 Redis | 远程 Redis |
|------|------------|------------|
| **性能** | 最快 | 网络延迟 |
| **安全性** | 基础 | 高安全 |
| **可用性** | 单点 | 高可用 |
| **成本** | 低 | 高 |
| **维护** | 简单 | 复杂 |

### 开发 vs 生产环境

| 方面 | 开发环境 | 生产环境 |
|------|----------|----------|
| **配置** | 简单 | 复杂 |
| **安全** | 基础 | 高安全 |
| **监控** | 基础 | 全面 |
| **备份** | 手动 | 自动 |
| **扩展** | 单机 | 集群 |

## 💡 最佳实践

### 1. Redis 连接管理
```python
class RedisConnectionManager:
    """Redis 连接管理器"""
    
    def __init__(self, config):
        self.config = config
        self.connection = None
        self.retry_count = 3
    
    def connect(self):
        """建立连接"""
        for attempt in range(self.retry_count):
            try:
                # 建立 Redis 连接
                self.connection = redis.Redis(**self.config)
                # 测试连接
                self.connection.ping()
                return True
            except Exception as e:
                print(f"连接尝试 {attempt + 1} 失败: {e}")
                if attempt < self.retry_count - 1:
                    time.sleep(1)
        return False
    
    def disconnect(self):
        """断开连接"""
        if self.connection:
            self.connection.close()
            self.connection = None
```

### 2. Redis 数据备份
```python
def backup_redis_data():
    """备份 Redis 数据"""
    redis_config = {
        "host": "localhost",
        "port": 6379,
        "db": 0
    }
    
    # 连接 Redis
    r = redis.Redis(**redis_config)
    
    # 获取所有键
    keys = r.keys("*")
    
    # 备份数据
    backup_data = {}
    for key in keys:
        backup_data[key] = r.get(key)
    
    # 保存备份
    with open("redis_backup.json", "w") as f:
        json.dump(backup_data, f)
    
    return backup_data
```

### 3. Redis 性能监控
```python
def monitor_redis_performance():
    """监控 Redis 性能"""
    redis_config = {
        "host": "localhost",
        "port": 6379,
        "db": 0
    }
    
    r = redis.Redis(**redis_config)
    
    # 获取性能信息
    info = r.info()
    
    performance_metrics = {
        'used_memory': info.get('used_memory', 0),
        'used_memory_peak': info.get('used_memory_peak', 0),
        'connected_clients': info.get('connected_clients', 0),
        'total_commands_processed': info.get('total_commands_processed', 0),
        'keyspace_hits': info.get('keyspace_hits', 0),
        'keyspace_misses': info.get('keyspace_misses', 0)
    }
    
    return performance_metrics
```

### 4. Redis 故障恢复
```python
def redis_failover_recovery():
    """Redis 故障恢复"""
    primary_config = {
        "host": "redis-primary.com",
        "port": 6379,
        "db": 0
    }
    
    backup_config = {
        "host": "redis-backup.com",
        "port": 6379,
        "db": 0
    }
    
    # 尝试主服务器
    try:
        store = MCPStore.setup_store(**primary_config)
        return store
    except Exception as e:
        print(f"主服务器连接失败: {e}")
    
    # 尝试备份服务器
    try:
        store = MCPStore.setup_store(**backup_config)
        print("已切换到备份服务器")
        return store
    except Exception as e:
        print(f"备份服务器连接失败: {e}")
        raise Exception("所有 Redis 服务器都不可用")
```

## 🔧 常见问题

### Q1: 如何选择 Redis 配置？
**A**: 
- 开发环境：本地 Redis，简单配置
- 测试环境：本地 Redis，基础配置
- 生产环境：远程 Redis，安全配置

### Q2: Redis 连接失败怎么办？
**A**: 
- 检查网络连接
- 验证认证信息
- 检查防火墙设置
- 确认 Redis 服务状态

### Q3: 如何优化 Redis 性能？
**A**: 
- 使用连接池
- 启用持久化
- 配置内存限制
- 监控性能指标

### Q4: Redis 数据如何备份？
**A**: 
- 定期备份数据
- 使用 Redis 持久化
- 配置主从复制
- 实施灾难恢复

### Q5: 如何监控 Redis 状态？
**A**: 
- 监控连接数
- 监控内存使用
- 监控命令执行
- 监控错误率

## 🔗 相关文档

- [Redis 支持文档](../../../mcpstore_docs/docs/database/redis.md)
- [Redis 配置文档](../../../mcpstore_docs/docs/database/redis.md#配置)
- [Redis 使用示例文档](../../../mcpstore_docs/docs/database/redis.md#使用示例)
- [Redis 最佳实践文档](../../../mcpstore_docs/docs/database/redis.md#最佳实践)

