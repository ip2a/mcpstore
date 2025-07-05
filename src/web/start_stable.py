#!/usr/bin/env python3
"""
MCPStore Web界面稳定启动脚本
解决RuntimeError和其他常见问题
"""

import subprocess
import sys
import os
import time

def main():
    """稳定启动函数"""
    print("🚀 启动MCPStore Web界面 (稳定版)...")
    
    # 设置工作目录
    web_dir = os.path.dirname(os.path.abspath(__file__))
    os.chdir(web_dir)
    
    # 检查核心依赖
    try:
        import streamlit
        print(f"✅ Streamlit {streamlit.__version__} 已安装")
    except ImportError:
        print("❌ 请安装 Streamlit: pip install streamlit")
        sys.exit(1)
    
    try:
        import requests
        print("✅ Requests 已安装")
    except ImportError:
        print("❌ 请安装 Requests: pip install requests")
        sys.exit(1)
    
    # 启动Streamlit - 使用稳定配置
    cmd = [
        sys.executable, "-m", "streamlit", "run", "app.py",
        "--server.port", "8501",
        "--server.address", "localhost",
        "--server.fileWatcherType", "none",  # 禁用文件监控
        "--server.runOnSave", "false",       # 禁用自动重载
        "--logger.level", "error",           # 减少日志输出
        "--client.showErrorDetails", "false" # 隐藏错误详情
    ]
    
    print(f"🌐 启动地址: http://localhost:8501")
    print("📝 配置说明:")
    print("  - 禁用文件监控 (避免RuntimeError)")
    print("  - 禁用自动重载 (提高稳定性)")
    print("  - 减少日志输出 (清洁控制台)")
    print("按 Ctrl+C 停止服务")
    print("-" * 50)
    
    try:
        # 启动进程
        process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
            universal_newlines=True
        )
        
        # 等待启动
        print("⏳ 正在启动服务...")
        time.sleep(3)
        
        # 检查进程状态
        if process.poll() is None:
            print("✅ 服务启动成功！")
            print("🌐 请访问: http://localhost:8501")
            
            # 等待进程结束
            process.wait()
        else:
            print("❌ 服务启动失败")
            return_code = process.returncode
            print(f"返回码: {return_code}")
            
    except KeyboardInterrupt:
        print("\n👋 正在停止服务...")
        try:
            process.terminate()
            process.wait(timeout=5)
        except:
            process.kill()
        print("✅ 服务已停止")
    except Exception as e:
        print(f"❌ 启动失败: {e}")

if __name__ == "__main__":
    main()
