import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from utils.import_helper import setup_import_path
setup_import_path()

import os
from langchain.agents import create_agent
from langchain_openai import ChatOpenAI

from mcpstore import MCPStore


store = MCPStore.setup_store(debug=True)


demo_mcp = {
  "mcpServers": {
    "playwright": {
      "command": "npx",
       "args": ["@playwright/mcp", "--isolated"]
          }
  }
}

store.for_store().add_service(demo_mcp)
store.for_store().wait_service("playwright",timeout=30)


tools = store.for_store().for_langchain().list_tools()

print("loaded langchain tools:", len(tools))

llm = ChatOpenAI(
    temperature=0,
    model=os.getenv("OPENAI_MODEL", "deepseek-chat"),
    openai_api_key=os.getenv("OPENAI_API_KEY", ""),
    openai_api_base=os.getenv("OPENAI_API_BASE", "https://api.deepseek.com"),
)

agent_graph = create_agent(
model=llm,
tools=tools,
system_prompt="你是一个助手，回答的时候带上表情",
)
events = agent_graph.stream({"messages": [{"role": "user", "content":  "打开百度，搜索白色电风扇"}]})
for event in events:
    event_type = list(event.keys())[0]
    event_data = event[event_type]
    print(f"\n[事件类型: {event_type}]")
    print(event_data)



