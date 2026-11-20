import os
from pathlib import Path

from mcp.server.fastmcp import FastMCP


mcp = FastMCP("mcpstore-cli-fixture")
label = os.getenv("MCPSTORE_FIXTURE_LABEL")
call_log_path = os.getenv("MCPSTORE_FIXTURE_CALL_LOG")


def labeled(default: str) -> str:
    return f"{label}: {default}" if label else default


def log_call(entry: str) -> None:
    if call_log_path:
        with open(call_log_path, "a", encoding="utf-8") as handle:
            handle.write(entry + "\n")


@mcp.tool()
def greet(name: str) -> str:
    """Greet caller."""
    log_call("tool:greet")
    return labeled(f"Hello, {name}!")


@mcp.resource("fixture://docs/readme")
def readme() -> str:
    """Fixture README resource."""
    log_call("resource:readme")
    return labeled("This is the MCPStore fixture resource.")


if os.getenv("MCPSTORE_FIXTURE_TEMPLATE"):

    @mcp.resource("fixture://docs/{name}")
    def document(name: str) -> str:
        """Fixture resource template."""
        return labeled(f"Fixture document: {name}")


@mcp.prompt()
def explain(topic: str = "overview") -> str:
    """Fixture prompt."""
    log_call("prompt:explain")
    return labeled(f"Explain {topic} via fixture prompt.")


# Catalog-scale items need per-index searchable text: BM25 tokenizes on word
# runs, and bare digits ("3") are single-char tokens that get dropped, so a
# query like "extra tool 3" would tie across all items. Number words give each
# item a unique token to rank on.
_NUMBER_WORDS = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
]


def _index_word(index: int) -> str:
    return _NUMBER_WORDS[index % len(_NUMBER_WORDS)]


def extra_tool(index: int):
    def _tool(query: str = "x"):
        log_call(f"tool:extra_tool_{index}")
        return labeled(f"extra tool {index}: {query}")

    _tool.__name__ = f"extra_tool_{index}"
    _tool.__doc__ = f"Extra fixture tool number {_index_word(index)} for catalog scale."
    return mcp.tool()(_tool)


def extra_prompt(index: int):
    def _prompt(topic: str = f"extra{index}") -> str:
        log_call(f"prompt:extra_prompt_{index}")
        return labeled(f"Explain extra topic {index}: {topic}")

    _prompt.__name__ = f"extra_prompt_{index}"
    _prompt.__doc__ = f"Extra fixture prompt number {_index_word(index)} for catalog scale."
    return mcp.prompt()(_prompt)


def extra_resource(index: int):
    uri = f"fixture://docs/extra{index}"

    def _read() -> str:
        log_call(f"resource:extra{index}")
        return labeled(f"Extra fixture resource {index}.")

    return mcp.resource(uri)(_read)


def extra_template(index: int):
    uri = f"fixture://tpl{index}/{{name}}"

    def _read(name: str) -> str:
        log_call(f"template:{uri}:{name}")
        return labeled(f"Fixture template {index} document: {name}")

    return mcp.resource(uri)(_read)


if os.getenv("MCPSTORE_FIXTURE_COLLISION"):

    @mcp.tool()
    def search_tools(query: str = "x") -> str:
        """Real upstream tool that happens to own the helper name."""
        log_call("tool:search_tools")
        return labeled(f"upstream search_tools: {query}")


# Catalog-scale knobs for the gateway search tests: each extra item widens the
# visible catalog of its kind so the per-kind activation thresholds can be
# exercised end to end.
for _index in range(int(os.getenv("MCPSTORE_FIXTURE_EXTRA_TOOLS", "0"))):
    extra_tool(_index)
for _index in range(int(os.getenv("MCPSTORE_FIXTURE_EXTRA_PROMPTS", "0"))):
    extra_prompt(_index)
for _index in range(int(os.getenv("MCPSTORE_FIXTURE_EXTRA_RESOURCES", "0"))):
    extra_resource(_index)
for _index in range(int(os.getenv("MCPSTORE_FIXTURE_TEMPLATES", "0"))):
    extra_template(_index)


if __name__ == "__main__":
    if pid_file := os.getenv("MCPSTORE_FIXTURE_PID_FILE"):
        Path(pid_file).write_text(str(os.getpid()), encoding="utf-8")
    mcp.run("stdio")
