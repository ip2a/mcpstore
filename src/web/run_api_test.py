#!/usr/bin/env python3
"""
运行API测试脚本
快速验证新添加的API接口功能
"""

import sys
import os

# 添加当前目录到Python路径
current_dir = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, current_dir)

def main():
    """主函数"""
    print("🚀 MCPStore Web API 测试启动")
    print("=" * 50)
    
    try:
        # 导入测试模块
        from test_new_apis import main as run_tests
        
        # 运行测试
        run_tests()
        
    except ImportError as e:
        print(f"❌ 导入错误: {e}")
        print("请确保所有依赖模块都已正确安装")
        
    except Exception as e:
        print(f"❌ 运行错误: {e}")
        import traceback
        traceback.print_exc()
    
    print("\n" + "=" * 50)
    print("🏁 测试完成")

if __name__ == "__main__":
    main()
