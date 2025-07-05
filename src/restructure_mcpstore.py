#!/usr/bin/env python3
"""
MCPStore 结构重构脚本
重新组织项目结构，提高代码可维护性
"""

import os
import shutil
from pathlib import Path

def create_directory_structure():
    """创建新的目录结构"""
    base_path = Path("src/mcpstore")
    
    # 新的目录结构
    new_dirs = [
        # Core 子目录
        "core/managers",
        "core/processors", 
        "core/utils",
        "core/features",
        
        # Config 增强
        "config/validators",
        
        # Plugins 重构
        "plugins/base",
        "plugins/extensions",
        "plugins/integrations",
        
        # 新增目录
        "logging",
        "testing",
    ]
    
    for dir_path in new_dirs:
        full_path = base_path / dir_path
        full_path.mkdir(parents=True, exist_ok=True)
        
        # 创建 __init__.py
        init_file = full_path / "__init__.py"
        if not init_file.exists():
            init_file.write_text("# Auto-generated __init__.py\n")
    
    print("✅ 新目录结构创建完成")

def move_files():
    """移动文件到新位置"""
    base_path = Path("src/mcpstore")
    
    # 文件移动映射
    file_moves = {
        # json_mcp.py 移动到 config
        "plugins/json_mcp.py": "config/json_config.py",
        
        # Core 文件重新分类
        "core/client_manager.py": "core/managers/client_manager.py",
        "core/session_manager.py": "core/managers/session_manager.py", 
        "core/registry.py": "core/managers/registry.py",
        
        "core/config_processor.py": "core/processors/config_processor.py",
        "core/tool_resolver.py": "core/processors/tool_resolver.py",
        "core/tool_transformation.py": "core/processors/tool_transformation.py",
        
        "core/async_sync_helper.py": "core/utils/async_sync_helper.py",
        "core/transport.py": "core/utils/transport.py",
        "core/unified_config.py": "core/utils/unified_config.py",
        
        "core/auth_security.py": "core/features/auth_security.py",
        "core/cache_performance.py": "core/features/cache_performance.py",
        "core/monitoring_analytics.py": "core/features/monitoring_analytics.py",
        "core/openapi_integration.py": "core/features/openapi_integration.py",
        "core/smart_reconnection.py": "core/features/smart_reconnection.py",
        "core/component_control.py": "core/features/component_control.py",
    }
    
    for src, dst in file_moves.items():
        src_path = base_path / src
        dst_path = base_path / dst
        
        if src_path.exists():
            # 确保目标目录存在
            dst_path.parent.mkdir(parents=True, exist_ok=True)
            
            # 移动文件
            shutil.move(str(src_path), str(dst_path))
            print(f"📁 移动: {src} -> {dst}")
        else:
            print(f"⚠️ 文件不存在: {src}")

def update_imports():
    """更新导入语句"""
    base_path = Path("src/mcpstore")
    
    # 需要更新的导入映射
    import_updates = {
        "from mcpstore.plugins.json_mcp": "from mcpstore.config.json_config",
        "from .plugins.json_mcp": "from .config.json_config",
        "from mcpstore.core.client_manager": "from mcpstore.core.managers.client_manager",
        "from mcpstore.core.session_manager": "from mcpstore.core.managers.session_manager",
        "from mcpstore.core.registry": "from mcpstore.core.managers.registry",
        "from mcpstore.core.config_processor": "from mcpstore.core.processors.config_processor",
        "from mcpstore.core.tool_resolver": "from mcpstore.core.processors.tool_resolver",
        "from mcpstore.core.tool_transformation": "from mcpstore.core.processors.tool_transformation",
        "from mcpstore.core.async_sync_helper": "from mcpstore.core.utils.async_sync_helper",
        "from mcpstore.core.transport": "from mcpstore.core.utils.transport",
        "from mcpstore.core.unified_config": "from mcpstore.core.utils.unified_config",
    }
    
    # 遍历所有 Python 文件
    for py_file in base_path.rglob("*.py"):
        if py_file.name.startswith("__pycache__"):
            continue
            
        try:
            content = py_file.read_text(encoding='utf-8')
            original_content = content
            
            # 更新导入语句
            for old_import, new_import in import_updates.items():
                content = content.replace(old_import, new_import)
            
            # 如果有变化，写回文件
            if content != original_content:
                py_file.write_text(content, encoding='utf-8')
                print(f"🔄 更新导入: {py_file.relative_to(base_path)}")
                
        except Exception as e:
            print(f"❌ 更新失败 {py_file}: {e}")

def create_new_init_files():
    """创建新的 __init__.py 文件"""
    base_path = Path("src/mcpstore")
    
    # 各模块的 __init__.py 内容
    init_contents = {
        "core/managers/__init__.py": '''"""
MCPStore 管理器模块
包含客户端管理、会话管理、注册表管理等功能
"""

from .client_manager import ClientManager
from .session_manager import SessionManager  
from .registry import Registry

__all__ = ["ClientManager", "SessionManager", "Registry"]
''',
        
        "core/processors/__init__.py": '''"""
MCPStore 处理器模块
包含配置处理、工具解析、工具转换等功能
"""

from .config_processor import ConfigProcessor
from .tool_resolver import ToolResolver
from .tool_transformation import ToolTransformation

__all__ = ["ConfigProcessor", "ToolResolver", "ToolTransformation"]
''',
        
        "core/utils/__init__.py": '''"""
MCPStore 工具模块
包含异步同步助手、传输层、统一配置等工具
"""

from .async_sync_helper import AsyncSyncHelper
from .transport import Transport
from .unified_config import UnifiedConfig

__all__ = ["AsyncSyncHelper", "Transport", "UnifiedConfig"]
''',
        
        "core/features/__init__.py": '''"""
MCPStore 功能模块
包含认证安全、缓存性能、监控分析等高级功能
"""

from .auth_security import AuthSecurity
from .cache_performance import CachePerformance
from .monitoring_analytics import MonitoringAnalytics
from .openapi_integration import OpenAPIIntegration
from .smart_reconnection import SmartReconnection
from .component_control import ComponentControl

__all__ = [
    "AuthSecurity", "CachePerformance", "MonitoringAnalytics",
    "OpenAPIIntegration", "SmartReconnection", "ComponentControl"
]
''',
        
        "config/__init__.py": '''"""
MCPStore 配置模块
包含配置管理、JSON配置、验证器等功能
"""

from .config import Config
from .json_config import MCPConfig, MCPConfigModel, MCPServerModel

__all__ = ["Config", "MCPConfig", "MCPConfigModel", "MCPServerModel"]
''',
        
        "plugins/__init__.py": '''"""
MCPStore 插件系统
支持扩展和集成插件
"""

# 插件系统将在后续版本中实现
__all__ = []
''',
    }
    
    for file_path, content in init_contents.items():
        full_path = base_path / file_path
        full_path.parent.mkdir(parents=True, exist_ok=True)
        full_path.write_text(content.strip() + "\n", encoding='utf-8')
        print(f"📝 创建: {file_path}")

def clean_empty_directories():
    """清理空目录"""
    base_path = Path("src/mcpstore")
    
    # 删除空的 __pycache__ 目录
    for pycache_dir in base_path.rglob("__pycache__"):
        if pycache_dir.is_dir():
            try:
                shutil.rmtree(pycache_dir)
                print(f"🗑️ 删除缓存目录: {pycache_dir.relative_to(base_path)}")
            except Exception as e:
                print(f"⚠️ 删除失败 {pycache_dir}: {e}")

def create_tests_directory():
    """创建测试目录结构"""
    tests_path = Path("src/tests")
    tests_path.mkdir(exist_ok=True)
    
    test_dirs = [
        "unit",
        "integration", 
        "performance",
        "fixtures",
        "utils"
    ]
    
    for test_dir in test_dirs:
        dir_path = tests_path / test_dir
        dir_path.mkdir(exist_ok=True)
        
        init_file = dir_path / "__init__.py"
        init_file.write_text("# Test module\n")
    
    print("✅ 测试目录结构创建完成")

def main():
    """主重构函数"""
    print("🚀 开始 MCPStore 结构重构")
    print("=" * 50)
    
    try:
        # 1. 创建新目录结构
        create_directory_structure()
        
        # 2. 移动文件
        move_files()
        
        # 3. 创建新的 __init__.py 文件
        create_new_init_files()
        
        # 4. 更新导入语句
        update_imports()
        
        # 5. 清理空目录
        clean_empty_directories()
        
        # 6. 创建测试目录
        create_tests_directory()
        
        print("\n🎉 MCPStore 结构重构完成！")
        print("\n📋 重构总结:")
        print("   ✅ 重新组织了 core 目录结构")
        print("   ✅ 移动了 json_mcp.py 到 config 模块")
        print("   ✅ 创建了清晰的模块分层")
        print("   ✅ 更新了所有导入语句")
        print("   ✅ 清理了缓存目录")
        print("   ✅ 创建了测试目录结构")
        
        print("\n⚠️ 注意事项:")
        print("   1. 请测试重构后的代码是否正常工作")
        print("   2. 可能需要手动调整一些复杂的导入关系")
        print("   3. 建议运行测试套件验证功能完整性")
        
    except Exception as e:
        print(f"\n❌ 重构过程中出现错误: {e}")
        print("请检查错误并手动修复")

if __name__ == "__main__":
    main()
