from unittest import TestCase
from unittest.mock import Mock, patch

from mcpstore import MCPStore
from mcpstore.config import FileConfig, RedisConfig


class StoreSetupDefaultsTests(TestCase):
    def _setup(self, source=None, mode=None):
        backend = Mock()
        with patch.object(MCPStore, "setup", return_value=backend) as setup:
            result = MCPStore.setup_store(source=source, mode=mode)
        self.assertIs(result, backend)
        return setup.call_args.kwargs

    def test_defaults_to_local_file_control_plane(self):
        options = self._setup()
        self.assertIsInstance(options["source"], FileConfig)
        self.assertEqual(options["source_mode"], "local")
        self.assertEqual(options["node_mode"], "control_plane")

    def test_redis_still_defaults_to_control_plane(self):
        source = RedisConfig(url="redis://localhost:6379/0")
        options = self._setup(source)
        self.assertIs(options["source"], source)
        self.assertEqual(options["source_mode"], "db")
        self.assertEqual(options["node_mode"], "control_plane")

    def test_data_plane_must_be_explicit(self):
        source = RedisConfig(url="redis://localhost:6379/0")
        options = self._setup(source, "data_plane")
        self.assertEqual(options["source_mode"], "db")
        self.assertEqual(options["node_mode"], "data_plane")

    def test_invalid_mode_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "mode must be one of"):
            self._setup(FileConfig(), "automatic")
