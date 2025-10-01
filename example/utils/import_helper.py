"""
导入路径配置助手
优先使用本地 src/mcpstore，如果不存在则使用环境中的 mcpstore
"""

import sys
from pathlib import Path


def setup_import_path():
    """
    配置导入路径，优先使用本地 src/mcpstore
    
    返回:
        bool: True 表示使用本地，False 表示使用环境
    """
    try:
        # 计算项目根目录（example/utils -> example -> project_root）
        current_file = Path(__file__).resolve()
        project_root = current_file.parent.parent.parent
        src_path = project_root / "src"
        
        if src_path.exists():
            # 将 src 路径插入到 sys.path 最前面
            sys.path.insert(0, str(src_path))
            print(f"✅ 使用本地 mcpstore: {src_path}")
            return True
        else:
            print("⚠️ 本地 src/mcpstore 不存在，使用环境中的 mcpstore")
            return False
    except Exception as e:
        print(f"⚠️ 路径配置警告: {e}")
        print("   将尝试使用环境中的 mcpstore")
        return False

