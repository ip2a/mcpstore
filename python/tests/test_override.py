from __future__ import annotations

from typing import Any

from mcpstore.context.prompt import Prompt
from mcpstore.context.resource import Resource, ResourceTemplate
from mcpstore.context.tool import Tool


class NativeComponent:
    def __init__(self) -> None:
        self.override: dict[str, Any] | None = None
        self.enabled = True

    def info(self) -> dict[str, Any]:
        return {"name": "component"}

    def get_override(self) -> dict[str, Any] | None:
        return self.override

    def set_override(self, patch: dict[str, Any]) -> dict[str, Any]:
        self.override = dict(patch)
        return self.override

    def delete_override(self) -> None:
        self.override = None

    def enable(self) -> None:
        self.enabled = True

    def disable(self) -> None:
        self.enabled = False

    def get(self, arguments: dict[str, Any]) -> dict[str, Any]:
        return {"arguments": arguments}

    def read(self) -> dict[str, Any]:
        return {"contents": []}


def test_tool_override_forward_lifecycle() -> None:
    tool = Tool(NativeComponent())
    assert tool.get_override() is None
    assert tool.set_override({"display_name": "friendly"})["display_name"] == "friendly"
    assert tool.get_override() == {"display_name": "friendly"}
    tool.disable()
    tool.enable()
    tool.delete_override()
    assert tool.get_override() is None


def test_prompt_resource_and_template_forward_methods() -> None:
    prompt_native = NativeComponent()
    assert Prompt(prompt_native).get({"x": 1}) == {"arguments": {"x": 1}}
    assert Prompt(prompt_native).set_override({"enabled": False}) == {"enabled": False}

    resource = Resource(NativeComponent())
    assert resource.read() == {"contents": []}
    resource.set_override({"mime_type": "text/plain"})

    template = ResourceTemplate(NativeComponent())
    template.set_override({"description": "templated"})
    assert template.get_override() == {"description": "templated"}
