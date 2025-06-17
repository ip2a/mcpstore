#!/usr/bin/env python
"""
MCPStore包使用示例

本示例展示如何直接使用MCPStore包的核心功能：
1. 初始化核心组件
2. 注册store服务（开店）
3. 注册agent（用户注册）
4. 获取工具列表
5. 调用工具
"""

import asyncio
import json
import os
from typing import Dict, Any, List, Optional, Tuple
import logging

# 导入mcpstore包的核心组件
from mcpstore.core.orchestrator import MCPOrchestrator
from mcpstore.core.registry import ServiceRegistry
from mcpstore.plugins.json_mcp import MCPConfig
from mcpstore.core.store import MCPStore
from mcpstore.core.models.tool import ToolExecutionRequest, ToolInfo
from mcpstore.core.models.client import ClientRegistrationRequest
from mcpstore.core.models.common import RegistrationResponse
from mcpstore import MCPStore

# 配置日志
logging.basicConfig(level=logging.INFO, format='%(name)s:%(message)s')
logger = logging.getLogger(__name__)

async def main():
    """主函数：演示MCPStore的完整使用流程"""
    
    print("===== MCPStore包使用示例 =====")
    
    # 1. 初始化MCPStore核心组件
    print("\n1. 初始化MCPStore核心组件")
    store = MCPStore.setup_store()
    print("   ✓ 核心组件初始化完成")
    
    # 2. 注册Store服务
    print("\n2. 注册Store服务")
    try:
        # Store模式：全量注册所有服务
        registration_result = await store.register_json_service(
            client_id=store.client_manager.main_client_id
        )
        
        if registration_result.success:
            logger.info("========================================")
            logger.info("🎉 Store服务注册成功！")
            logger.info(f"注册了 {len(registration_result.service_names)} 个服务:")
            for service_name in registration_result.service_names:
                logger.info(f"  - {service_name}")
            logger.info("========================================")
        else:
            logger.error(f"Store服务注册失败: {registration_result.message}")
            
    except Exception as e:
        logger.error(f"Store服务注册过程中发生错误: {e}")
    
    # 3. Agent注册流程
    print("\n===== 开始Agent注册流程 =====")
    try:
        # Agent模式：注册指定服务
        agent_id = "demo_agent_001"
        
        # 获取可用服务列表
        available_services = await store.list_services()
        if available_services:
            # 选择前2个服务进行Agent注册
            selected_services = [s.name for s in available_services[:2]]
            
            agent_registration = await store.register_json_service(
                client_id=agent_id,
                service_names=selected_services
            )
            
            if agent_registration.success:
                print(f"✓ Agent {agent_id} 注册成功")
                print(f"  注册的服务: {agent_registration.service_names}")
            else:
                print(f"✗ Agent注册失败: {agent_registration.message}")
        else:
            print("✗ 没有可用的服务进行Agent注册")
            
    except Exception as e:
        logger.error(f"发生未知错误: {e}")
    
    # 4. 获取工具列表
    print("\n4. 获取工具列表")
    try:
        # Store级别的工具列表
        store_tools = await store.for_store().list_tools()
        print(f"   Store级别工具数量: {len(store_tools)}")
        
        # Agent级别的工具列表
        agent_tools = await store.for_agent(agent_id).list_tools()
        print(f"   Agent级别工具数量: {len(agent_tools)}")
        
        # 显示前几个工具
        if store_tools:
            print("   前5个Store工具:")
            for i, tool in enumerate(store_tools[:5]):
                print(f"     {i+1}. {tool.name}")
                
    except Exception as e:
        logger.error(f"获取工具列表失败: {e}")
    
    # 5. 工具调用示例
    print("\n5. 工具调用示例")
    try:
        if store_tools:
            # 选择第一个工具进行测试
            test_tool = store_tools[0]
            print(f"   测试工具: {test_tool.name}")
            
            # 构造测试参数（这里需要根据具体工具调整）
            test_args = {}
            if hasattr(test_tool, 'inputSchema') and test_tool.inputSchema:
                properties = test_tool.inputSchema.get('properties', {})
                for prop_name, prop_info in properties.items():
                    # 为每个参数提供默认测试值
                    if prop_info.get('type') == 'string':
                        test_args[prop_name] = "测试值"
                    elif prop_info.get('type') == 'number':
                        test_args[prop_name] = 1
                    elif prop_info.get('type') == 'boolean':
                        test_args[prop_name] = True
            
            if test_args:
                result = await store.for_store().use_tool(test_tool.name, test_args)
                print(f"   工具调用结果: {str(result)[:100]}...")
            else:
                print("   跳过工具调用（无法构造测试参数）")
                
    except Exception as e:
        logger.error(f"工具调用失败: {e}")
    
    print("\n===== 示例完成 =====")

if __name__ == "__main__":
    asyncio.run(main())
