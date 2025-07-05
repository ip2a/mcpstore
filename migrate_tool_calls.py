#!/usr/bin/env python3
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
    pattern1 = r'use_tool\s*\(\s*["']([^"']+)_([^"']+)["']\s*,'
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
    
    print(f"\n📊 迁移完成:")
    print(f"   迁移文件数: {len(migrated_files)}")
    for file_path in migrated_files:
        print(f"   - {file_path}")

if __name__ == "__main__":
    print("🚀 开始 MCPStore 工具调用迁移...")
    migrate_project()
    print("\n✅ 迁移完成！")
    print("\n📝 迁移说明:")
    print("   1. 旧格式 'service_tool' 已转换为 'service__tool'")
    print("   2. 原文件已备份为 .backup 文件")
    print("   3. 建议测试迁移后的代码确保正常工作")
    print("   4. 确认无误后可删除 .backup 文件")
