"""
测试：LangChain 集成 - 工具链构建
功能：测试使用 LangChain 工具构建工具链
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore
import json

print("=" * 60)
print("测试：LangChain 集成 - 工具链构建")
print("=" * 60)

# 1️⃣ 初始化 Store 并添加服务
print("\n1️⃣ 初始化 Store 并添加服务")
store = MCPStore.setup_store(debug=True)
service_config = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(service_config)
store.for_store().wait_service("weather", timeout=30.0)
print(f"✅ 服务 'weather' 已添加并就绪")

# 2️⃣ 获取 LangChain 集成对象
print("\n2️⃣ 获取 LangChain 集成对象")
langchain_integration = store.for_langchain()
print(f"✅ LangChain 集成对象获取成功")

# 3️⃣ 获取 LangChain 工具列表
print("\n3️⃣ 获取 LangChain 工具列表")
langchain_tools = langchain_integration.list_tools()
print(f"✅ LangChain 工具列表获取成功")
print(f"   工具数量: {len(langchain_tools) if isinstance(langchain_tools, list) else 'N/A'}")

# 4️⃣ 构建简单工具链
print("\n4️⃣ 构建简单工具链")
if isinstance(langchain_tools, list) and langchain_tools:
    # 选择主要工具
    main_tool = langchain_tools[0]
    tool_name = getattr(main_tool, 'name', 'N/A')
    print(f"   主要工具: {tool_name}")
    
    # 构建工具链
    def simple_tool_chain(input_data):
        """简单工具链"""
        print(f"   工具链输入: {input_data}")
        
        # 步骤1: 调用主要工具
        step1_result = main_tool.func(input_data)
        print(f"   步骤1结果: {step1_result}")
        
        # 步骤2: 处理结果
        if isinstance(step1_result, str):
            step2_result = f"处理后的结果: {step1_result[:50]}..."
        else:
            step2_result = f"处理后的结果: {step1_result}"
        
        print(f"   步骤2结果: {step2_result}")
        
        # 步骤3: 生成最终结果
        final_result = {
            'input': input_data,
            'step1': step1_result,
            'step2': step2_result,
            'timestamp': time.time()
        }
        
        print(f"   最终结果: {final_result}")
        return final_result
    
    print(f"   ✅ 简单工具链构建成功")
else:
    print(f"   ❌ 无可用工具，无法构建工具链")
    exit()

# 5️⃣ 测试简单工具链
print("\n5️⃣ 测试简单工具链")
test_inputs = ["北京", "上海", "广州"]

for i, input_data in enumerate(test_inputs, 1):
    print(f"   测试 {i}: 输入='{input_data}'")
    try:
        result = simple_tool_chain(input_data)
        print(f"   ✅ 工具链执行成功")
        print(f"   结果类型: {type(result)}")
        print()
    except Exception as e:
        print(f"   ❌ 工具链执行失败: {e}")
        print()

# 6️⃣ 构建复杂工具链
print("\n6️⃣ 构建复杂工具链")
if len(langchain_tools) >= 2:
    # 选择多个工具
    tool1 = langchain_tools[0]
    tool2 = langchain_tools[1] if len(langchain_tools) > 1 else langchain_tools[0]
    
    print(f"   工具1: {getattr(tool1, 'name', 'N/A')}")
    print(f"   工具2: {getattr(tool2, 'name', 'N/A')}")
    
    # 构建复杂工具链
    def complex_tool_chain(input_data):
        """复杂工具链"""
        print(f"   复杂工具链输入: {input_data}")
        
        # 步骤1: 调用工具1
        step1_result = tool1.func(input_data)
        print(f"   步骤1结果: {step1_result}")
        
        # 步骤2: 调用工具2
        step2_result = tool2.func(input_data)
        print(f"   步骤2结果: {step2_result}")
        
        # 步骤3: 合并结果
        merged_result = {
            'tool1_result': step1_result,
            'tool2_result': step2_result,
            'input': input_data
        }
        
        print(f"   合并结果: {merged_result}")
        
        # 步骤4: 生成报告
        report = {
            'summary': f"工具链处理完成，输入: {input_data}",
            'details': merged_result,
            'timestamp': time.time()
        }
        
        print(f"   最终报告: {report}")
        return report
    
    print(f"   ✅ 复杂工具链构建成功")
    
    # 测试复杂工具链
    print(f"   测试复杂工具链:")
    try:
        result = complex_tool_chain("测试输入")
        print(f"   ✅ 复杂工具链执行成功")
        print(f"   结果类型: {type(result)}")
    except Exception as e:
        print(f"   ❌ 复杂工具链执行失败: {e}")
else:
    print(f"   ⚠️ 工具数量不足，无法构建复杂工具链")

# 7️⃣ 构建条件工具链
print("\n7️⃣ 构建条件工具链")
def conditional_tool_chain(input_data, condition):
    """条件工具链"""
    print(f"   条件工具链输入: {input_data}, 条件: {condition}")
    
    if condition == "weather":
        # 天气相关处理
        result = main_tool.func(input_data)
        processed_result = f"天气信息: {result}"
    elif condition == "location":
        # 位置相关处理
        result = main_tool.func(input_data)
        processed_result = f"位置信息: {result}"
    else:
        # 默认处理
        result = main_tool.func(input_data)
        processed_result = f"默认处理: {result}"
    
    print(f"   条件处理结果: {processed_result}")
    return processed_result

# 测试条件工具链
print(f"   测试条件工具链:")
test_conditions = ["weather", "location", "default"]

for condition in test_conditions:
    print(f"   条件: {condition}")
    try:
        result = conditional_tool_chain("测试数据", condition)
        print(f"   ✅ 条件工具链执行成功")
        print(f"   结果: {result}")
        print()
    except Exception as e:
        print(f"   ❌ 条件工具链执行失败: {e}")
        print()

# 8️⃣ 构建循环工具链
print("\n8️⃣ 构建循环工具链")
def loop_tool_chain(inputs):
    """循环工具链"""
    print(f"   循环工具链输入: {inputs}")
    results = []
    
    for i, input_data in enumerate(inputs):
        print(f"   循环 {i+1}: {input_data}")
        try:
            result = main_tool.func(input_data)
            results.append({
                'input': input_data,
                'result': result,
                'index': i
            })
            print(f"   ✅ 循环 {i+1} 成功")
        except Exception as e:
            print(f"   ❌ 循环 {i+1} 失败: {e}")
            results.append({
                'input': input_data,
                'error': str(e),
                'index': i
            })
    
    print(f"   循环结果: {results}")
    return results

# 测试循环工具链
print(f"   测试循环工具链:")
test_inputs = ["北京", "上海", "广州", "深圳"]
try:
    result = loop_tool_chain(test_inputs)
    print(f"   ✅ 循环工具链执行成功")
    print(f"   结果数量: {len(result)}")
except Exception as e:
    print(f"   ❌ 循环工具链执行失败: {e}")

# 9️⃣ 工具链性能测试
print("\n9️⃣ 工具链性能测试")
import time

def performance_tool_chain(input_data):
    """性能测试工具链"""
    start_time = time.time()
    
    # 执行工具链
    result = main_tool.func(input_data)
    
    end_time = time.time()
    execution_time = end_time - start_time
    
    return {
        'result': result,
        'execution_time': execution_time
    }

print(f"   性能测试:")
for i in range(3):
    start_time = time.time()
    result = performance_tool_chain("性能测试")
    end_time = time.time()
    
    print(f"   测试 {i+1}: {result['execution_time']:.4f}秒")

# 🔟 工具链特性总结
print("\n🔟 工具链特性总结")
print(f"   LangChain 工具链特性:")
print(f"   - 支持简单工具链")
print(f"   - 支持复杂工具链")
print(f"   - 支持条件工具链")
print(f"   - 支持循环工具链")
print(f"   - 支持性能监控")

print("\n💡 工具链构建特点:")
print("   - 灵活的工具组合")
print("   - 支持条件逻辑")
print("   - 支持循环处理")
print("   - 支持错误处理")
print("   - 支持性能监控")

print("\n💡 使用场景:")
print("   - 复杂工作流")
print("   - 自动化流程")
print("   - 数据处理管道")
print("   - 业务逻辑实现")
print("   - 系统集成")

print("\n" + "=" * 60)
print("✅ LangChain 集成 - 工具链构建测试完成")
print("=" * 60)

