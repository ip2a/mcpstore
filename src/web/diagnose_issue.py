#!/usr/bin/env python3
"""
诊断页面显示问题
"""

import sys
import os

# 添加当前目录到Python路径
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

def test_basic_imports():
    """测试基本导入"""
    print("🧪 测试基本导入...")
    
    try:
        import streamlit as st
        print(f"✅ Streamlit {st.__version__} 导入成功")
        
        import app
        print("✅ app.py 导入成功")
        
        # 测试关键函数
        functions = ['main', 'render_header', 'render_sidebar', 'render_main_content']
        for func in functions:
            if hasattr(app, func):
                print(f"✅ {func} 函数存在")
            else:
                print(f"❌ {func} 函数缺失")
                return False
        
        return True
    except Exception as e:
        print(f"❌ 导入失败: {e}")
        return False

def test_page_modules():
    """测试页面模块"""
    print("\n🧪 测试页面模块...")
    
    try:
        from pages import service_management, tool_management, agent_management, monitoring, configuration
        print("✅ 所有页面模块导入成功")
        
        # 测试每个模块是否有show方法
        modules = [
            ('service_management', service_management),
            ('tool_management', tool_management),
            ('agent_management', agent_management),
            ('monitoring', monitoring),
            ('configuration', configuration)
        ]
        
        for name, module in modules:
            if hasattr(module, 'show'):
                print(f"✅ {name}.show() 方法存在")
            else:
                print(f"❌ {name}.show() 方法缺失")
                return False
        
        return True
    except Exception as e:
        print(f"❌ 页面模块测试失败: {e}")
        return False

def test_config_manager():
    """测试配置管理器"""
    print("\n🧪 测试配置管理器...")
    
    try:
        from utils.config_manager import SessionManager, WebConfigManager
        print("✅ 配置管理器导入成功")
        
        # 测试基本功能
        config_manager = WebConfigManager()
        print("✅ WebConfigManager 创建成功")
        
        return True
    except Exception as e:
        print(f"❌ 配置管理器测试失败: {e}")
        return False

def test_api_client():
    """测试API客户端"""
    print("\n🧪 测试API客户端...")
    
    try:
        from utils.api_client import MCPStoreAPI
        print("✅ API客户端导入成功")
        
        # 测试创建客户端
        api_client = MCPStoreAPI("http", "http://localhost:18611")
        print("✅ API客户端创建成功")
        
        return True
    except Exception as e:
        print(f"❌ API客户端测试失败: {e}")
        return False

def check_file_syntax():
    """检查文件语法"""
    print("\n🧪 检查文件语法...")
    
    files_to_check = ['app.py', 'style.py']
    
    for file_path in files_to_check:
        try:
            with open(file_path, 'r', encoding='utf-8') as f:
                content = f.read()
            
            # 尝试编译
            compile(content, file_path, 'exec')
            print(f"✅ {file_path} 语法正确")
        except SyntaxError as e:
            print(f"❌ {file_path} 语法错误: {e}")
            return False
        except Exception as e:
            print(f"❌ {file_path} 检查失败: {e}")
            return False
    
    return True

def test_streamlit_config():
    """测试Streamlit配置"""
    print("\n🧪 测试Streamlit配置...")
    
    try:
        config_path = '.streamlit/config.toml'
        if os.path.exists(config_path):
            with open(config_path, 'r', encoding='utf-8') as f:
                content = f.read()
            print("✅ Streamlit配置文件存在")
            
            # 检查关键配置
            if 'fileWatcherType = "none"' in content:
                print("✅ 文件监控已禁用")
            else:
                print("⚠️ 文件监控未禁用")
            
            return True
        else:
            print("⚠️ Streamlit配置文件不存在")
            return True
    except Exception as e:
        print(f"❌ Streamlit配置测试失败: {e}")
        return False

def create_minimal_test():
    """创建最小测试文件"""
    print("\n🧪 创建最小测试文件...")
    
    minimal_app = '''
import streamlit as st

def main():
    st.title("🚀 MCPStore 测试")
    st.write("如果您能看到这个页面，说明基本功能正常。")
    
    # 侧边栏测试
    with st.sidebar:
        st.header("侧边栏测试")
        if st.button("测试按钮"):
            st.success("按钮点击成功！")
    
    # 主内容测试
    st.header("主内容区域")
    st.info("这是一个最小化的测试页面")
    
    col1, col2 = st.columns(2)
    with col1:
        st.metric("测试指标1", 100)
    with col2:
        st.metric("测试指标2", 200)

if __name__ == "__main__":
    main()
'''
    
    try:
        with open('test_minimal.py', 'w', encoding='utf-8') as f:
            f.write(minimal_app)
        print("✅ 最小测试文件已创建: test_minimal.py")
        print("   运行命令: streamlit run test_minimal.py")
        return True
    except Exception as e:
        print(f"❌ 创建测试文件失败: {e}")
        return False

def main():
    """主诊断函数"""
    print("🔍 MCPStore Web页面问题诊断")
    print("=" * 50)
    
    tests = [
        ("基本导入", test_basic_imports),
        ("页面模块", test_page_modules),
        ("配置管理器", test_config_manager),
        ("API客户端", test_api_client),
        ("文件语法", check_file_syntax),
        ("Streamlit配置", test_streamlit_config),
        ("创建测试文件", create_minimal_test)
    ]
    
    passed = 0
    total = len(tests)
    
    for test_name, test_func in tests:
        try:
            if test_func():
                passed += 1
                print(f"✅ {test_name} 正常")
            else:
                print(f"❌ {test_name} 异常")
        except Exception as e:
            print(f"❌ {test_name} 错误: {e}")
        
        print("-" * 30)
    
    print(f"\n📊 诊断结果: {passed}/{total} 正常")
    
    if passed >= 5:
        print("🎉 大部分功能正常！")
        print("\n💡 建议:")
        print("  1. 尝试运行: streamlit run test_minimal.py")
        print("  2. 如果最小测试正常，问题可能在复杂逻辑中")
        print("  3. 检查浏览器控制台是否有JavaScript错误")
        print("  4. 尝试清除浏览器缓存")
    else:
        print("⚠️ 发现多个问题，需要逐一解决。")
    
    return passed >= 5

if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)
