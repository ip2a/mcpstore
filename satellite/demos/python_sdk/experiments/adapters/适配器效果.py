"""
MCPStore adapter usage example
Demonstrates integrating MCPStore with framework adapters (OpenAI, LangChain, AutoGen, etc.)
"""

from mcpstore import MCPStore




# Initialize the store
store = MCPStore.setup_store(debug=False)
store_ctx = store.for_store()

# Add the sample service
store.for_store().add_service({
    "name": "demo_adapt",
    "url": "http://115.190.220.103:2230/mcp"
})

# 1. OpenAI adapter example
openai_tools = store_ctx.for_openai().list_tools()
print(openai_tools)
# 2. LangChain adapter example
langchain_tools = store_ctx.for_langchain().list_tools()
print(langchain_tools)
# 3. AutoGen adapter example
autogen_functions = store_ctx.for_autogen().get_functions()
print(autogen_functions)

# 4. LlamaIndex adapter example
llamaindex_tools = store_ctx.for_llamaindex().list_tools()
print(llamaindex_tools)

# 5. Semantic Kernel adapter example
sk_functions = store_ctx.for_semantic_kernel().get_functions()
print(sk_functions)


