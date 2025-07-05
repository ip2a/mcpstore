#!/usr/bin/env python3
"""
MCPStore Web界面启动脚本 v2.0
增强版启动器，支持多种启动模式和配置
"""

import subprocess
import sys
import os
import argparse
import json
from pathlib import Path

def check_dependencies():
    """检查依赖包"""
    required_packages = [
        'streamlit',
        'requests',
        'pandas'
    ]

    missing_packages = []

    for package in required_packages:
        try:
            __import__(package)
        except ImportError:
            missing_packages.append(package)

    if missing_packages:
        print(f"❌ 缺少依赖包: {', '.join(missing_packages)}")
        print("请运行: pip install -r requirements.txt")
        return False

    print("✅ 核心依赖包检查通过")
    return True

def check_optional_dependencies():
    """检查可选依赖包"""
    optional_packages = {
        'plotly': '图表增强',
        'ujson': 'JSON性能优化',
        'pydantic': '数据验证',
        'cachetools': '缓存功能'
    }

    available_features = []
    missing_features = []

    for package, description in optional_packages.items():
        try:
            __import__(package)
            available_features.append(f"✅ {description}")
        except ImportError:
            missing_features.append(f"⚠️ {description} (缺少 {package})")

    if available_features:
        print("🎯 可用增强功能:")
        for feature in available_features:
            print(f"  {feature}")

    if missing_features:
        print("💡 可选功能 (可通过安装依赖启用):")
        for feature in missing_features:
            print(f"  {feature}")

def setup_environment():
    """设置环境"""
    web_dir = Path(__file__).parent
    os.chdir(web_dir)

    # 创建必要的目录
    (web_dir / "logs").mkdir(exist_ok=True)
    (web_dir / "data").mkdir(exist_ok=True)

    # 设置环境变量
    os.environ.setdefault("STREAMLIT_BROWSER_GATHER_USAGE_STATS", "false")
    os.environ.setdefault("STREAMLIT_SERVER_HEADLESS", "true")

    return web_dir

def create_streamlit_config(web_dir: Path, args):
    """创建Streamlit配置文件"""
    config_dir = web_dir / ".streamlit"
    config_dir.mkdir(exist_ok=True)

    config_content = f"""
[server]
port = {args.port}
address = "{args.host}"
headless = true

[browser]
gatherUsageStats = false

[theme]
primaryColor = "#1f77b4"
backgroundColor = "#ffffff"
secondaryBackgroundColor = "#f0f2f6"
textColor = "#262730"

[logger]
level = "{'DEBUG' if args.debug else 'INFO'}"
"""

    config_file = config_dir / "config.toml"
    with open(config_file, 'w') as f:
        f.write(config_content)

    print(f"📝 Streamlit配置已创建: {config_file}")

def start_streamlit(args):
    """启动Streamlit应用"""
    cmd = [
        sys.executable, "-m", "streamlit", "run", "app.py",
        "--server.port", str(args.port),
        "--server.address", args.host,
        "--browser.gatherUsageStats", "false"
    ]

    if args.debug:
        cmd.extend(["--logger.level", "debug"])

    print(f"🚀 启动命令: {' '.join(cmd)}")
    print(f"🌐 访问地址: http://{args.host}:{args.port}")
    print("按 Ctrl+C 停止服务")
    print("-" * 50)

    try:
        subprocess.run(cmd)
    except KeyboardInterrupt:
        print("\n👋 MCPStore Web界面已停止")
    except Exception as e:
        print(f"❌ 启动失败: {e}")
        sys.exit(1)

def show_system_info():
    """显示系统信息"""
    print("📊 系统信息:")
    print(f"  Python版本: {sys.version}")
    print(f"  工作目录: {os.getcwd()}")
    print(f"  平台: {sys.platform}")

def main():
    """主函数"""
    parser = argparse.ArgumentParser(
        description="MCPStore Web界面启动器 v2.0",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
示例用法:
  python run.py                    # 默认启动
  python run.py --port 8502        # 指定端口
  python run.py --debug            # 调试模式
  python run.py --check-only       # 仅检查依赖
        """
    )

    parser.add_argument(
        "--port", "-p",
        type=int,
        default=8501,
        help="Web服务端口 (默认: 8501)"
    )

    parser.add_argument(
        "--host", "-H",
        default="0.0.0.0",
        help="绑定地址 (默认: 0.0.0.0)"
    )

    parser.add_argument(
        "--debug", "-d",
        action="store_true",
        help="启用调试模式"
    )

    parser.add_argument(
        "--check-only", "-c",
        action="store_true",
        help="仅检查依赖，不启动服务"
    )

    parser.add_argument(
        "--info", "-i",
        action="store_true",
        help="显示系统信息"
    )

    args = parser.parse_args()

    print("🚀 MCPStore Web管理界面启动器 v2.0")
    print("=" * 50)

    if args.info:
        show_system_info()
        print("-" * 50)

    # 检查依赖
    if not check_dependencies():
        sys.exit(1)

    # 检查可选依赖
    check_optional_dependencies()

    if args.check_only:
        print("✅ 依赖检查完成")
        return

    print("-" * 50)

    # 设置环境
    web_dir = setup_environment()

    # 创建配置
    create_streamlit_config(web_dir, args)

    # 启动应用
    start_streamlit(args)

if __name__ == "__main__":
    main()
