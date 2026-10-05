from unittest import TestCase
from unittest.mock import Mock, patch

from mcpstore import MCPStore
from mcpstore.config import FileConfig, RedisConfig


class StoreSetupDefaultsTests(TestCase):
    def _setup(self, source=None, panel=None):
        backend = Mock()
        with patch.object(MCPStore, "setup", return_value=backend) as setup:
            result = MCPStore.setup_store(source=source, panel=panel)
        self.assertIs(result, backend)
        return setup.call_args.kwargs

    def test_defaults_to_local_file(self):
        options = self._setup()
        self.assertIsInstance(options["source"], FileConfig)
        self.assertNotIn("source_mode", options, "unified model: source_mode is gone")

    def test_redis_source_passes_through(self):
        source = RedisConfig(url="redis://localhost:6379/0")
        options = self._setup(source=source)
        self.assertIs(options["source"], source)
        self.assertNotIn("source_mode", options, "unified model: backend is a deployment parameter")
