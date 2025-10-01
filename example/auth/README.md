# 权限认证测试模块

本模块包含权限认证相关的测试文件。

## 📋 测试文件列表

| 文件名 | 说明 | 上下文 |
|--------|------|--------|
| `test_store_auth_basic.py` | 基础认证测试 | Store 级别 |
| `test_store_auth_advanced.py` | 高级认证测试 | Store 级别 |

## 🚀 运行测试

### 运行单个测试

```bash
# 基础认证测试
python example/auth/test_store_auth_basic.py

# 高级认证测试
python example/auth/test_store_auth_advanced.py
```

### 运行所有认证测试

```bash
# Windows
for %f in (example\auth\test_*.py) do python %f

# Linux/Mac
for f in example/auth/test_*.py; do python "$f"; done
```

## 📝 测试说明

### 1. 基础认证测试
测试基础认证功能：
- 用户名密码认证
- 服务级认证
- 配置管理
- 权限控制
- 错误处理

### 2. 高级认证测试
测试高级认证功能：
- OAuth 2.0 认证
- JWT 令牌支持
- 角色权限控制
- 令牌自动刷新
- 状态监控

## 💡 核心概念

### 认证类型

| 类型 | 特点 | 用途 | 示例 |
|------|------|------|------|
| **基础认证** | 用户名密码 | 简单系统 | Basic Auth |
| **OAuth 2.0** | 标准协议 | 企业系统 | OAuth 2.0 |
| **JWT** | 令牌认证 | 微服务 | JWT Token |
| **API Key** | 密钥认证 | API 服务 | API Key |

### 权限级别

| 级别 | 权限 | 用途 | 示例 |
|------|------|------|------|
| **用户** | 基础权限 | 普通用户 | 读取数据 |
| **管理员** | 管理权限 | 系统管理 | 读写数据 |
| **超级管理员** | 完全权限 | 系统维护 | 所有操作 |

## 🎯 使用场景

### 场景 1：基础认证配置
```python
# 基础认证配置
def setup_basic_auth():
    auth_config = {
        "authentication": {
            "enabled": True,
            "type": "basic",
            "username": "admin",
            "password": "secure_password"
        }
    }
    
    store = MCPStore.setup_store(**auth_config)
    return store
```

### 场景 2：OAuth 2.0 认证
```python
# OAuth 2.0 认证配置
def setup_oauth_auth():
    auth_config = {
        "authentication": {
            "enabled": True,
            "type": "oauth",
            "client_id": "your_client_id",
            "client_secret": "your_client_secret",
            "token_url": "https://auth.example.com/token",
            "scope": "read write"
        }
    }
    
    store = MCPStore.setup_store(**auth_config)
    return store
```

### 场景 3：JWT 令牌认证
```python
# JWT 令牌认证配置
def setup_jwt_auth():
    auth_config = {
        "authentication": {
            "enabled": True,
            "type": "jwt",
            "jwt": {
                "enabled": True,
                "secret_key": "your_secret_key",
                "algorithm": "HS256",
                "expiration": 3600
            }
        }
    }
    
    store = MCPStore.setup_store(**auth_config)
    return store
```

### 场景 4：角色权限控制
```python
# 角色权限控制
def check_user_permissions(user_role, operation):
    permissions = {
        "user": ["read"],
        "admin": ["read", "write"],
        "super_admin": ["read", "write", "delete", "admin"]
    }
    
    user_permissions = permissions.get(user_role, [])
    return operation in user_permissions

# 使用示例
if check_user_permissions("admin", "write"):
    print("允许写入操作")
else:
    print("拒绝写入操作")
```

## 📊 认证对比

### 基础认证 vs 高级认证

| 方面 | 基础认证 | 高级认证 |
|------|----------|----------|
| **复杂度** | 简单 | 复杂 |
| **安全性** | 基础 | 高 |
| **标准性** | 基础 | 标准 |
| **扩展性** | 有限 | 强 |
| **维护** | 简单 | 复杂 |

### 认证协议对比

| 协议 | 特点 | 适用场景 | 安全性 |
|------|------|----------|--------|
| **Basic Auth** | 简单 | 内部系统 | 基础 |
| **OAuth 2.0** | 标准 | 企业系统 | 高 |
| **JWT** | 轻量 | 微服务 | 高 |
| **API Key** | 简单 | API 服务 | 中等 |

## 💡 最佳实践

### 1. 认证配置管理
```python
class AuthConfigManager:
    """认证配置管理器"""
    
    def __init__(self):
        self.configs = {}
    
    def add_auth_config(self, name, config):
        """添加认证配置"""
        self.configs[name] = config
    
    def get_auth_config(self, name):
        """获取认证配置"""
        return self.configs.get(name)
    
    def validate_auth_config(self, config):
        """验证认证配置"""
        required_fields = ["enabled", "type"]
        for field in required_fields:
            if field not in config:
                raise ValueError(f"缺少必填字段: {field}")
        return True
```

### 2. 令牌管理
```python
class TokenManager:
    """令牌管理器"""
    
    def __init__(self, config):
        self.config = config
        self.tokens = {}
    
    def generate_token(self, user_id, roles):
        """生成令牌"""
        import time
        
        token_data = {
            "user_id": user_id,
            "roles": roles,
            "issued_at": time.time(),
            "expires_at": time.time() + self.config.get("expiration", 3600)
        }
        
        # 生成 JWT 令牌
        token = self._create_jwt_token(token_data)
        self.tokens[token] = token_data
        
        return token
    
    def validate_token(self, token):
        """验证令牌"""
        if token not in self.tokens:
            return False
        
        token_data = self.tokens[token]
        if time.time() > token_data["expires_at"]:
            del self.tokens[token]
            return False
        
        return True
    
    def refresh_token(self, token):
        """刷新令牌"""
        if not self.validate_token(token):
            return None
        
        token_data = self.tokens[token]
        new_token = self.generate_token(
            token_data["user_id"],
            token_data["roles"]
        )
        
        del self.tokens[token]
        return new_token
```

### 3. 权限控制
```python
class PermissionManager:
    """权限管理器"""
    
    def __init__(self):
        self.permissions = {
            "user": ["read"],
            "admin": ["read", "write"],
            "super_admin": ["read", "write", "delete", "admin"]
        }
    
    def check_permission(self, user_role, operation):
        """检查权限"""
        user_permissions = self.permissions.get(user_role, [])
        return operation in user_permissions
    
    def get_user_permissions(self, user_role):
        """获取用户权限"""
        return self.permissions.get(user_role, [])
    
    def add_permission(self, role, permission):
        """添加权限"""
        if role not in self.permissions:
            self.permissions[role] = []
        
        if permission not in self.permissions[role]:
            self.permissions[role].append(permission)
    
    def remove_permission(self, role, permission):
        """移除权限"""
        if role in self.permissions and permission in self.permissions[role]:
            self.permissions[role].remove(permission)
```

### 4. 认证监控
```python
class AuthMonitor:
    """认证监控器"""
    
    def __init__(self):
        self.auth_logs = []
        self.failed_attempts = {}
    
    def log_auth_attempt(self, user_id, success, details):
        """记录认证尝试"""
        log_entry = {
            "timestamp": time.time(),
            "user_id": user_id,
            "success": success,
            "details": details
        }
        
        self.auth_logs.append(log_entry)
        
        if not success:
            if user_id not in self.failed_attempts:
                self.failed_attempts[user_id] = 0
            self.failed_attempts[user_id] += 1
    
    def get_failed_attempts(self, user_id):
        """获取失败尝试次数"""
        return self.failed_attempts.get(user_id, 0)
    
    def is_user_locked(self, user_id, max_attempts=5):
        """检查用户是否被锁定"""
        return self.get_failed_attempts(user_id) >= max_attempts
    
    def reset_failed_attempts(self, user_id):
        """重置失败尝试次数"""
        if user_id in self.failed_attempts:
            del self.failed_attempts[user_id]
```

## 🔧 常见问题

### Q1: 如何选择认证类型？
**A**: 
- 基础认证：简单系统、内部使用
- OAuth 2.0：企业系统、标准协议
- JWT：微服务、无状态认证
- API Key：API 服务、简单认证

### Q2: 如何管理令牌过期？
**A**: 
- 设置合理的过期时间
- 实现自动刷新机制
- 监控令牌状态
- 处理过期异常

### Q3: 如何实现权限控制？
**A**: 
- 定义角色和权限
- 实现权限检查
- 控制资源访问
- 记录权限日志

### Q4: 如何监控认证状态？
**A**: 
- 记录认证日志
- 监控失败尝试
- 跟踪令牌使用
- 生成安全报告

### Q5: 如何提高认证安全性？
**A**: 
- 使用强密码策略
- 启用多因素认证
- 实施访问控制
- 定期安全审计

## 🔗 相关文档

- [认证概览文档](../../../mcpstore_docs/docs/authentication/overview.md)
- [认证配置文档](../../../mcpstore_docs/docs/authentication/configuration.md)
- [认证示例文档](../../../mcpstore_docs/docs/authentication/examples.md)
- [认证API参考文档](../../../mcpstore_docs/docs/authentication/api-reference.md)

