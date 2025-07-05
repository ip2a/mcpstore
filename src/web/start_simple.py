#!/usr/bin/env python3
"""
MCPStore Web界面简化启动脚本
用于快速测试和调试
"""

import subprocess
import sys
import os

def main():
    """简化启动函数"""
    print("🚀 启动MCPStore Web界面 (简化版)...")
    
    # 设置工作目录
    web_dir = os.path.dirname(os.path.abspath(__file__))
    os.chdir(web_dir)
    
    # 检查核心依赖
    try:
        import streamlit
        print("✅ Streamlit 已安装")
    except ImportError:
        print("❌ 请安装 Streamlit: pip install streamlit")
        sys.exit(1)
    
    try:
        import requests
        print("✅ Requests 已安装")
    except ImportError:
        print("❌ 请安装 Requests: pip install requests")
        sys.exit(1)
    
    # 启动Streamlit
    cmd = [
        sys.executable, "-m", "streamlit", "run", "app.py",
        "--server.port", "8501",
        "--server.address", "localhost",
        "--server.fileWatcherType", "none"  # 禁用文件监控以避免RuntimeError
    ]
    
    print(f"🌐 启动地址: http://localhost:8501")
    print("按 Ctrl+C 停止服务")
    print("-" * 40)
    
    try:
        subprocess.run(cmd)
    except KeyboardInterrupt:
        print("\n👋 服务已停止")
    except Exception as e:
        print(f"❌ 启动失败: {e}")

if __name__ == "__main__":
    main()
