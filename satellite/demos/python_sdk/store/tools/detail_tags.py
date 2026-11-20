"""
Test: fetching tool tags from the Store
Purpose: exercise tool_tags() to retrieve a tool's tags
Context: Store level
"""

# MCPStore example import setup
import sys
sys.path.insert(0, "../..")
from example_utils import setup_example_import
setup_example_import()  # Set up the import path automatically



from mcpstore import MCPStore

print("=" * 60)
print("Test: fetching tool tags from the Store")
print("=" * 60)

# 1️⃣ Initialize the Store and add a service
print("\n1️⃣ Initialize the Store and add a service")
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
print(f" Service 'weather' added and ready")

# 2️⃣ Find the tool
print("\n2️⃣ Find the tool")
tool_name = "get_current_weather"
tool_proxy = store.for_store().find_tool(tool_name)
print(f" Tool found: {tool_name}")

# 3️⃣ Fetch tool tags with tool_tags()
print("\n3️⃣ Fetch tool tags with tool_tags()")
tags = tool_proxy.tool_tags()
print(f" Tool tags retrieved successfully")
print(f"   Return type: {type(tags)}")
print(f"   Tags: {tags}")

# 4️⃣ Inspect the tag contents
print("\n4️⃣ Inspect the tag contents")
if tags:
    print(f"📋 Tool tags:")
    if isinstance(tags, list):
        for tag in tags:
            print(f"   - {tag}")
    elif isinstance(tags, dict):
        for key, value in tags.items():
            print(f"   - {key}: {value}")
    else:
        print(f"   Tag contents: {tags}")
else:
    print(f"   (no tags)")

# 5️⃣ Fetch tags for multiple tools
print("\n5️⃣ Fetch tags for multiple tools")
tools = store.for_store().list_tools()
if tools:
    print(f"📋 Tool tag overview:")
    for tool in tools[:5]:
        proxy = store.for_store().find_tool(tool.name)
        tool_tags = proxy.tool_tags()
        print(f"   {tool.name}: {tool_tags if tool_tags else '(no tags)'}")
    
    if len(tools) > 5:
        print(f"   ... and {len(tools) - 5} more tools")

# 6️⃣ Group tools by tag
print("\n6️⃣ Group tools by tag")
tag_groups = {}
for tool in tools:
    proxy = store.for_store().find_tool(tool.name)
    tool_tags = proxy.tool_tags()
    
    if tool_tags:
        if isinstance(tool_tags, list):
            for tag in tool_tags:
                if tag not in tag_groups:
                    tag_groups[tag] = []
                tag_groups[tag].append(tool.name)
        elif isinstance(tool_tags, str):
            if tool_tags not in tag_groups:
                tag_groups[tool_tags] = []
            tag_groups[tool_tags].append(tool.name)

if tag_groups:
    print(f"📊 Grouped by tag:")
    for tag, tool_names in tag_groups.items():
        print(f"   Tag '{tag}': {len(tool_names)} tools")
        for name in tool_names[:3]:
            print(f"      - {name}")
        if len(tool_names) > 3:
            print(f"      ... and {len(tool_names) - 3} more")
else:
    print(f"   (no tag groupings yet)")

# 7️⃣ What tags are good for
print("\n7️⃣ Practical uses of tags")
print(f"   Tags can be used for:")
print(f"   - Tool categorization and organization")
print(f"   - Tool search and filtering")
print(f"   - Permission control")
print(f"   - Grouped UI display")
print(f"   - Tool recommendations")

print("\n💡 tool_tags() characteristics:")
print("   - Returns a tool's tags")
print("   - May be a list, a string, or a dict")
print("   - Used for tool categorization and organization")
print("   - Supports tool search and filtering")
print("   - Well suited to metadata management")

print("\n💡 Typical scenarios:")
print("   - Tool categorization")
print("   - Tag-based search")
print("   - Permission control")
print("   - Grouped UI display")
print("   - Tool recommendation systems")

print("\n" + "=" * 60)
print(" Store tool-tag retrieval test complete")
print("=" * 60)

