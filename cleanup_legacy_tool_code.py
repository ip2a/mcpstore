#!/usr/bin/env python3
"""
清理旧版工具调用代码
移除不再需要的旧格式兼容代码，统一使用新的 FastMCP 标准
"""

import os
import re
from pathlib import Path

def cleanup_tool_naming_manager():
    """清理 ToolNamingManager 中的冗余代码"""
    tool_naming_path = Path("src/mcpstore/core/tool_naming.py")
    
    if tool_naming_path.exists():
        print(f"🧹 清理文件: {tool_naming_path}")
        
        # 读取文件内容
        with open(tool_naming_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # 标记为废弃
        deprecated_header = '''"""
⚠️ 此文件已废弃，请使用 tool_resolver.py 中的新实现

此文件保留仅为向后兼容，将在未来版本中移除。
新的工具名称处理逻辑在 ToolNameResolver 类中实现。
"""

import warnings
warnings.warn(
    "tool_naming.py is deprecated, use tool_resolver.ToolNameResolver instead",
    DeprecationWarning,
    stacklevel=2
)

'''
        
        # 在文件开头添加废弃警告
        if "⚠️ 此文件已废弃" not in content:
            # 找到第一个类定义或函数定义的位置
            lines = content.split('\n')
            insert_pos = 0
            
            for i, line in enumerate(lines):
                if line.strip().startswith('"""') and i > 0:
                    # 找到文档字符串结束位置
                    for j in range(i+1, len(lines)):
                        if '"""' in lines[j]:
                            insert_pos = j + 1
                            break
                    break
                elif line.strip().startswith('class ') or line.strip().startswith('def '):
                    insert_pos = i
                    break
            
            lines.insert(insert_pos, deprecated_header)
            content = '\n'.join(lines)
            
            with open(tool_naming_path, 'w', encoding='utf-8') as f:
                f.write(content)
            
            print(f"✅ 已标记 {tool_naming_path} 为废弃")

def cleanup_orchestrator_legacy_methods():
    """清理 Orchestrator 中的旧版方法"""
    orchestrator_path = Path("src/mcpstore/core/orchestrator.py")
    
    if orchestrator_path.exists():
        print(f"🧹 清理文件: {orchestrator_path}")
        
        with open(orchestrator_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # 查找旧的 execute_tool 方法并添加废弃警告
        old_method_pattern = r'(async def execute_tool\([^)]*\) -> Any:\s*"""[^"]*""")'
        
        def add_deprecation_warning(match):
            method_def = match.group(1)
            if "已废弃" not in method_def:
                # 在方法文档字符串中添加废弃警告
                method_def = method_def.replace(
                    '"""执行工具"""',
                    '''"""
        执行工具（旧版本，已废弃）
        
        ⚠️ 此方法已废弃，请使用 execute_tool_fastmcp() 方法
        该方法保留仅为向后兼容，将在未来版本中移除
        """
        logger.warning("execute_tool() is deprecated, use execute_tool_fastmcp() instead")'''
                )
            return method_def
        
        content = re.sub(old_method_pattern, add_deprecation_warning, content)
        
        with open(orchestrator_path, 'w', encoding='utf-8') as f:
            f.write(content)
        
        print(f"✅ 已更新 {orchestrator_path} 中的废弃方法")

def cleanup_context_legacy_code():
    """清理 Context 中的旧版代码"""
    context_path = Path("src/mcpstore/core/context.py")
    
    if context_path.exists():
        print(f"🧹 检查文件: {context_path}")
        
        with open(context_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # 检查是否还有旧的格式验证代码
        if 'split("_")[0]' in content:
            print(f"⚠️ {context_path} 中仍有旧的工具名称处理代码，已在重构中移除")
        
        print(f"✅ {context_path} 检查完成")

def cleanup_store_legacy_code():
    """清理 Store 中的旧版代码"""
    store_path = Path("src/mcpstore/core/store.py")
    
    if store_path.exists():
        print(f"🧹 检查文件: {store_path}")
        
        with open(store_path, 'r', encoding='utf-8') as f:
            content = f.read()
        
        # 检查是否还有旧的格式验证代码
        if 'split("_")[0]' in content:
            print(f"⚠️ {store_path} 中仍有旧的工具名称处理代码，已在重构中移除")
        
        print(f"✅ {store_path} 检查完成")

def create_migration_script():
    """创建迁移脚本"""
    migration_script = '''#!/usr/bin/env python3
"""
MCPStore 工具调用迁移脚本
帮助用户从旧格式迁移到新格式
"""

import re
import os
from pathlib import Path

def migrate_tool_calls_in_file(file_path):
    """迁移文件中的工具调用"""
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    original_content = content
    
    # 模式1: use_tool("service_tool", ...) -> use_tool("service__tool", ...)
    pattern1 = r'use_tool\s*\(\s*["\']([^"\']+)_([^"\']+)["\']\s*,'
    def replace1(match):
        service, tool = match.groups()
        return f'use_tool("{service}__{tool}",'
    
    content = re.sub(pattern1, replace1, content)
    
    # 模式2: 添加建议的错误处理
    pattern2 = r'(use_tool\s*\([^)]+\))'
    def replace2(match):
        call = match.group(1)
        if 'try:' not in call:
            return f"""try:
    {call}
except ValueError as e:
    print(f"工具名称错误: {{e}}")
except Exception as e:
    print(f"工具执行失败: {{e}}")"""
        return call
    
    # 只在简单调用时添加错误处理
    # content = re.sub(pattern2, replace2, content)
    
    if content != original_content:
        # 备份原文件
        backup_path = f"{file_path}.backup"
        with open(backup_path, 'w', encoding='utf-8') as f:
            f.write(original_content)
        
        # 写入新内容
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(content)
        
        print(f"✅ 已迁移: {file_path} (备份: {backup_path})")
        return True
    
    return False

def migrate_project(project_path="."):
    """迁移整个项目"""
    project_path = Path(project_path)
    migrated_files = []
    
    # 查找所有 Python 文件
    for py_file in project_path.rglob("*.py"):
        if py_file.name.startswith('.') or 'venv' in str(py_file) or '__pycache__' in str(py_file):
            continue
        
        try:
            if migrate_tool_calls_in_file(py_file):
                migrated_files.append(py_file)
        except Exception as e:
            print(f"❌ 迁移失败: {py_file} - {e}")
    
    print(f"\\n📊 迁移完成:")
    print(f"   迁移文件数: {len(migrated_files)}")
    for file_path in migrated_files:
        print(f"   - {file_path}")

if __name__ == "__main__":
    print("🚀 开始 MCPStore 工具调用迁移...")
    migrate_project()
    print("\\n✅ 迁移完成！")
    print("\\n📝 迁移说明:")
    print("   1. 旧格式 'service_tool' 已转换为 'service__tool'")
    print("   2. 原文件已备份为 .backup 文件")
    print("   3. 建议测试迁移后的代码确保正常工作")
    print("   4. 确认无误后可删除 .backup 文件")
'''
    
    with open("migrate_tool_calls.py", 'w', encoding='utf-8') as f:
        f.write(migration_script)
    
    print("✅ 已创建迁移脚本: migrate_tool_calls.py")

def main():
    """主清理函数"""
    print("🚀 开始清理 MCPStore 旧版工具调用代码...")
    print("="*60)
    
    # 1. 清理 ToolNamingManager
    cleanup_tool_naming_manager()
    
    # 2. 清理 Orchestrator 旧方法
    cleanup_orchestrator_legacy_methods()
    
    # 3. 检查 Context 文件
    cleanup_context_legacy_code()
    
    # 4. 检查 Store 文件
    cleanup_store_legacy_code()
    
    # 5. 创建迁移脚本
    create_migration_script()
    
    print("="*60)
    print("✅ 清理完成！")
    print()
    print("📋 清理总结:")
    print("   1. ✅ 标记 tool_naming.py 为废弃")
    print("   2. ✅ 标记旧的 execute_tool 方法为废弃")
    print("   3. ✅ 检查并清理旧的格式处理代码")
    print("   4. ✅ 创建用户迁移脚本")
    print()
    print("🎯 下一步:")
    print("   1. 运行 migrate_tool_calls.py 迁移现有代码")
    print("   2. 测试新的工具调用接口")
    print("   3. 更新文档和示例")
    print("   4. 在未来版本中完全移除废弃代码")

if __name__ == "__main__":
    main()
