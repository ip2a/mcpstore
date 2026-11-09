from example_utils import setup_example_import, ExampleConfig

setup_example_import()

from mcpstore import MCPStore
from mcpstore.config import RedisConfig

redis_config = RedisConfig(**ExampleConfig.get_redis_config())


store = MCPStore.setup_store(
    source=redis_config,
    mode="data_plane",
    debug=True,
)
store.start_api_server(
    host='0.0.0.0',
    port=18200,
    show_startup_info=False,
    # log_level='warning'
)

