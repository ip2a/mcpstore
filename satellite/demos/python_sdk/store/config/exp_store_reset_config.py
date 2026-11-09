"""
Store configuration reset example
Demonstrates how to inspect and reset the MCPStore configuration,
including fetching config info and performing a full reset.
"""

from mcpstore import MCPStore

# Initialize the store
store = MCPStore.setup_store(debug=False)

# Fetch the various configuration views
json_config = store.get_json_config()
mcpjson_content = store.show_mcpjson()
data_space_info = store.get_data_space_info()

# Reset the configuration
reset_result = store.for_store().reset_config()
