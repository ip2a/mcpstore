
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()
from mcpstore import MCPStore
store = MCPStore.setup_store(debug=True)
# store = MCPStore.setup_store(mcp_json=r'S:\BaiduSyncdisk\2025_6\mcpstore\test_workspaces\workspace1\mcp.json', debug=False)
# store = MCPStore.setup_store(mcp_json=r'S:\BaiduSyncdisk\2025_6\mcpstore\test_workspaces\workspace1\mcp.json')

print('--')
l = store.show_mcpjson()
print(l)

print('重置mcpjson')
l = store.for_store().reset_config()
print(l)

print('--')
l = store.show_mcpjson()
print(l)
