#!/usr/bin/env python3
"""
批量修复 print 语句为 logger 调用
"""

import re
import os

def fix_print_statements_in_file(file_path):
    """修复文件中的 print 语句"""
    if not os.path.exists(file_path):
        print(f"文件不存在: {file_path}")
        return False
    
    with open(file_path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    original_content = content
    
    # 替换模式
    replacements = [
        # [INFO] -> logger.info
        (r'print\(f"\[INFO\]\[register_json_service\] ([^"]+)"\)', r'logger.info(f"\1")'),
        (r'print\("\[INFO\]\[register_json_service\] ([^"]+)"\)', r'logger.info("\1")'),
        
        # [ERROR] -> logger.error
        (r'print\(f"\[ERROR\]\[register_json_service\] ([^"]+)"\)', r'logger.error(f"\1")'),
        (r'print\("\[ERROR\]\[register_json_service\] ([^"]+)"\)', r'logger.error("\1")'),
        
        # [WARN] -> logger.warning
        (r'print\(f"\[WARN\]\[register_json_service\] ([^"]+)"\)', r'logger.warning(f"\1")'),
        (r'print\("\[WARN\]\[register_json_service\] ([^"]+)"\)', r'logger.warning("\1")'),
        
        # [DEBUG] -> logger.debug
        (r'print\(f"\[DEBUG\]\[register_json_service\] ([^"]+)"\)', r'logger.debug(f"\1")'),
        (r'print\("\[DEBUG\]\[register_json_service\] ([^"]+)"\)', r'logger.debug("\1")'),
        
        # 其他 add_service 相关的日志
        (r'print\(f"\[INFO\]\[add_service\] ([^"]+)"\)', r'logger.info(f"\1")'),
        (r'print\("\[INFO\]\[add_service\] ([^"]+)"\)', r'logger.info("\1")'),
        (r'print\(f"\[ERROR\]\[add_service\] ([^"]+)"\)', r'logger.error(f"\1")'),
        (r'print\("\[ERROR\]\[add_service\] ([^"]+)"\)', r'logger.error("\1")'),
        (r'print\(f"\[WARN\]\[add_service\] ([^"]+)"\)', r'logger.warning(f"\1")'),
        (r'print\("\[WARN\]\[add_service\] ([^"]+)"\)', r'logger.warning("\1")'),
        (r'print\(f"\[DEBUG\]\[add_service\] ([^"]+)"\)', r'logger.debug(f"\1")'),
        (r'print\("\[DEBUG\]\[add_service\] ([^"]+)"\)', r'logger.debug("\1")'),
    ]
    
    # 应用替换
    for pattern, replacement in replacements:
        content = re.sub(pattern, replacement, content)
    
    # 如果有变化，写回文件
    if content != original_content:
        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(content)
        print(f"✅ 修复了 {file_path}")
        return True
    else:
        print(f"⚪ {file_path} 无需修复")
        return False

def main():
    """主函数"""
    print("🔧 批量修复 print 语句")
    
    # 需要修复的文件列表
    files_to_fix = [
        "src/mcpstore/core/store.py",
        "src/mcpstore/core/context.py",
        "src/mcpstore/core/registry.py",
        "src/mcpstore/core/orchestrator.py",
        "src/mcpstore/core/client_manager.py",
        "src/mcpstore/core/session_manager.py",
    ]
    
    fixed_count = 0
    for file_path in files_to_fix:
        if fix_print_statements_in_file(file_path):
            fixed_count += 1
    
    print(f"\n🎉 修复完成！共修复了 {fixed_count} 个文件")

if __name__ == "__main__":
    main()
