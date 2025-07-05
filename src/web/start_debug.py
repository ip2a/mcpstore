#!/usr/bin/env python3
"""
调试启动脚本
"""

import subprocess
import sys
import os
import time

def main():
    """调试启动函数"""
    print("🚀 调试启动MCPStore Web界面...")
    print(f"Python版本: {sys.version}")
    print(f"工作目录: {os.getcwd()}")
    
    # 检查文件
    if os.path.exists('app.py'):
        print("✅ app.py 存在")
    else:
        print("❌ app.py 不存在")
        return
    
    # 检查依赖
    try:
        import streamlit
        print(f"✅ Streamlit版本: {streamlit.__version__}")
    except ImportError:
        print("❌ Streamlit未安装")
        return
    
    # 启动命令
    cmd = [
        sys.executable, "-m", "streamlit", "run", "app.py",
        "--server.port", "8501",
        "--server.address", "localhost",
        "--logger.level", "info"
    ]
    
    print(f"🌐 启动命令: {' '.join(cmd)}")
    print("🌐 访问地址: http://localhost:8501")
    print("=" * 50)
    
    # 启动进程
    process = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        universal_newlines=True
    )
    
    try:
        # 实时输出
        for line in process.stdout:
            print(line.rstrip())
            
            # 检查是否启动成功
            if "You can now view your Streamlit app in your browser" in line:
                print("🎉 Streamlit启动成功！")
            elif "Network URL:" in line:
                print("🌐 网络地址已就绪")
                
    except KeyboardInterrupt:
        print("\n👋 停止服务...")
        process.terminate()
        process.wait()

if __name__ == "__main__":
    main()
