from langchain.agents import create_tool_calling_agent, AgentExecutor
from langchain_core.prompts import ChatPromptTemplate
from langchain_openai import ChatOpenAI

from mcpstore import MCPStore

store = MCPStore.setup_store(debug=True)
store.for_store().add_service({"name":"mcpstore-wiki","url":"https://mcpstore.wiki/mcp"})
store.for_store().wait_service("mcpstore-wiki")
sls = store.for_store().list_services()
print(sls)
print(store.for_store().list_tools())
tools = store.for_store().for_langchain().list_tools()
print(tools)
llm = ChatOpenAI(
    temperature=0, model="deepseek-chat",
    openai_api_key="sk-24e1c752e6114950952365631d18cf4f",
    openai_api_base="https://api.deepseek.com"
)

prompt = ChatPromptTemplate.from_messages([
    ("system", "你是一个助手，回答的时候带上表情"),
    ("human", "{input}"),
    ("placeholder", "{agent_scratchpad}"),
])


agent = create_tool_calling_agent(llm, tools, prompt)
agent_executor = AgentExecutor(agent=agent, tools=tools, verbose=True)
query = "北京的天气怎么样？"
print(f"\n   🤔: {query}")
response = agent_executor.invoke({"input": query})
print(f"   🤖 : {response['output']}")


