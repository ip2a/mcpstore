from unittest import TestCase
from unittest.mock import Mock, patch

from mcpstore import MCPStore
from mcpstore.config import FileConfig, RedisConfig


class StoreSetupDefaultsTests(TestCase):
    def _setup(self, source=None):
        backend = Mock()
        with patch.object(MCPStore, "setup", return_value=backend) as setup:
            result = MCPStore.setup_store(source=source)
        self.assertIs(result, backend)
        return setup.call_args.kwargs

    def test_defaults_to_local_file(self):
        options = self._setup()
        self.assertIsInstance(options["source"], FileConfig)
        self.assertEqual(options["source_mode"], "local")

    def test_redis_resolves_to_db_source(self):
        source = RedisConfig(url="redis://localhost:6379/0")
        options = self._setup(source)
        self.assertIs(options["source"], source)
        self.assertEqual(options["source_mode"], "db")
