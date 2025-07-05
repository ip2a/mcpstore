#!/usr/bin/env python3
"""
修复导入问题的脚本
"""

import os
import re

def fix_file_imports(file_path):
    """修复单个文件的导入问题"""
    if not os.path.exists(file_path):
        return False
    
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # 检查是否需要添加typing导入
    needs_typing = False
    typing_imports = []
    
    # 检查是否使用了类型提示
    if re.search(r'-> (Dict|List|Optional|Any|Union)', content):
        needs_typing = True
        
        if 'Dict' in content:
            typing_imports.append('Dict')
        if 'List' in content:
            typing_imports.append('List')
        if 'Optional' in content:
            typing_imports.append('Optional')
        if 'Any' in content:
            typing_imports.append('Any')
        if 'Union' in content:
            typing_imports.append('Union')
    
    # 如果需要typing导入但没有导入
    if needs_typing and 'from typing import' not in content:
        # 找到第一个import语句的位置
        import_match = re.search(r'^import ', content, re.MULTILINE)
        if import_match:
            insert_pos = import_match.start()
            typing_import = f"from typing import {', '.join(set(typing_imports))}\n"
            content = content[:insert_pos] + typing_import + content[insert_pos:]
        else:
            # 如果没有import语句，在文件开头添加
            content = f"from typing import {', '.join(set(typing_imports))}\n" + content
    
    # 写回文件
    with open(file_path, 'w', encoding='utf-8') as f:
        f.write(content)
    
    return True

def main():
    """主函数"""
    print("🔧 修复MCPStore Web界面导入问题...")
    
    # 需要检查的文件列表
    files_to_check = [
        'app.py',
        'utils/api_client.py',
        'utils/config_manager.py',
        'utils/helpers.py',
        'components/ui_components.py',
        'components/service_components.py',
        'pages/service_management.py',
        'pages/tool_management.py',
        'pages/agent_management.py',
        'pages/monitoring.py',
        'pages/configuration.py'
    ]
    
    fixed_count = 0
    
    for file_path in files_to_check:
        if os.path.exists(file_path):
            try:
                if fix_file_imports(file_path):
                    print(f"✅ 已检查: {file_path}")
                    fixed_count += 1
                else:
                    print(f"⚠️ 跳过: {file_path}")
            except Exception as e:
                print(f"❌ 错误: {file_path} - {e}")
        else:
            print(f"⚠️ 文件不存在: {file_path}")
    
    print(f"\n📊 处理完成: {fixed_count} 个文件")
    print("🎯 建议运行测试: python test_basic.py")

if __name__ == "__main__":
    main()
