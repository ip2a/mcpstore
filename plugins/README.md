# MCPStore Client Plugins

This directory defines MCP client adapters as declarative plugins. Each plugin is a self-contained folder describing how to detect, configure, and integrate one coding client with MCPStore's gateway.

## Plugin Structure

Each plugin is a folder containing:

```
<client-id>/
  plugin.toml    # Required: manifest describing client integration
  icon.svg       # Optional: client icon
```

The folder name **must** match `plugin.id` in the manifest.

## Manifest Specification

Every `plugin.toml` must conform to this structure:

### `[plugin]` (required)

- **`id`** (string, required): Unique identifier matching folder name. Must match `^[a-z][a-z0-9-]*$`.
- **`label`** (string, required): Human-readable client name (English).
- **`spec_version`** (integer, required): Must be `1` for this spec version.
- **`description`** (string, optional): Brief description of the client.
- **`icon`** (string, optional): Relative path to icon file (must resolve inside plugin folder).

### `[config]` (required)

- **`format`** (string, required): Configuration file format. Must be `"json"` or `"toml"`.
- **`servers_key`** (string, required): Top-level key where MCP servers are stored (e.g., `"mcpServers"`, `"context_servers"`).
- **`supported_fields`** (array of strings, required): Field names accepted in server entries. Any other field causes import rejection.
- **`command_shape`** (string, optional, default `"string"`): How the command field is represented:
  - `"string"`: Single string value (Claude, Cursor, Codex).
  - `"array"`: Array of strings with first element as executable (OpenCode).
- **`validate_type_field`** (boolean, optional, default `false`): If `true`, enforce consistency between `type` field value and entry structure (OpenCode: `type="local"` requires `command`, `type="remote"` requires `url`).
- **`reject_disabled`** (boolean, optional, default `false`): If `true`, refuse to import entries with `enabled: false`.

### `[gateway_entry]` (optional)

If absent, this client does not support remote MCP servers (e.g., Claude Desktop's stdio-only limitation).

- **`url_key`** (string, optional, default `"url"`): Field name for gateway URL in generated entries.
- **`extra`** (table, optional): Additional fixed fields to include in gateway entries (e.g., OpenCode's `{ type = "remote", enabled = true }`).
- **`supports_remote`** (boolean, optional, default `true`): Whether client accepts remote servers. If `false`, gateway wiring fails with clear error.

### `[discover]` (at least one field required)

How to detect this client on the system:

- **`config_paths`** (array of strings, optional): Config file candidates in priority order. `~` is expanded. First existing file wins.
- **`config_paths_macos`** / **`config_paths_linux`** / **`config_paths_windows`** (array, optional): Platform-specific overrides for `config_paths`.
- **`config_dirs`** (array of strings, optional): Directories whose existence suggests the client. Fallback after config file search.
- **`config_dirs_macos`** / **`config_dirs_linux`** / **`config_dirs_windows`** (array, optional): Platform-specific overrides.
- **`cli_commands`** (array of strings, optional): CLI executables to search for on PATH. Last resort after files and directories.

At least one of `config_paths`, `config_dirs`, or `cli_commands` must be non-empty.

## Rules and Constraints

1. **Folder name = plugin ID**: Enforced at load time.
2. **Unknown fields are errors**: TOML parsing uses `deny_unknown_fields`. Typos fail loudly.
3. **Path rules**:
   - All `config_paths` and `config_dirs` must start with `~` or be absolute paths.
   - Relative paths are forbidden to prevent ambiguity.
   - Icon paths must resolve inside the plugin folder.
4. **Field validation**:
   - `id`: Must match `^[a-z][a-z0-9-]*$`.
   - `spec_version`: Must equal `1`.
   - `format`: Must be `"json"` or `"toml"`.
   - `command_shape`: Must be `"string"` or `"array"`.
5. **Import behavior**:
   - Server entries with fields not in `supported_fields` are rejected with named field list.
   - If `validate_type_field=true`, `type` field presence and value are checked against entry structure.
   - If `reject_disabled=true`, entries with `enabled: false` are skipped with clear message.
6. **Fail loud**: Malformed manifests, missing icon files, duplicate IDs, and incompatible `spec_version` all produce named errors. Plugins never silently fail.

## Built-in vs Third-party

- **Built-in plugins** (this directory): Embedded in the MCPStore binary via `include_str!()`.
- **Third-party plugins**: Future support will load from user directories like `~/.mcpstore/plugins/` using the same `ManifestSetup` code path.

Both use identical manifest format and validation logic.

## Validation

Before submitting:

1. Run `mcpstore plugin validate <plugin-folder>` to check manifest correctness.
2. Ensure the manifest matches `plugin.schema.json` (TODO: schema file).
3. Test that `icon` (if present) resolves and is valid SVG.

## Examples

See the built-in client folders (`claude/`, `codex/`, `opencode/`, `cursor/`, `pi/`, `omp/`) for reference implementations covering all format variations.

## Future Work

- `plugin.schema.json` for machine validation
- Multi-language labels (`label_zh`, `label_ja`)
- Plugin distribution channels (git, zip)
- User plugin directory scanning
