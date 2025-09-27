import time
from mcpstore import MCPStore

# 初始化商店
store = MCPStore.setup_store(debug=True)
print("-- 清理配置 --")
print(store.for_store().reset_config())
# 准备演示服务（与“测试_简单工具使用”一致）
demo_mcp = {
  "mcpServers": {
    "mcpstore-demo-weather": {
      "url": "https://mcpstore.wiki/mcp"
    }
  }
}

# 注册服务并等待连接
store.for_store().add_service(demo_mcp)
store.for_store().wait_service("mcpstore-demo-weather")

# 通过 find 获取服务代理
svc = store.for_store().find_service("mcpstore-demo-weather")

print("-- 服务详情 --")
info = svc.service_info()
print(info)

print("-- 清理配置 --")
print(store.for_store().reset_config())

