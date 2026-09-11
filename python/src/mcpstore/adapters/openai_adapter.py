# src/mcpstore/adapters/openai_adapter.py
"""
OpenAI adapter module.

Converts MCPStore tools into the OpenAI function calling format.
Compatible with langchain-openai's bind_tools and direct OpenAI API calls.
"""
from __future__ import annotations

import json
from typing import List, Dict, Any, Tuple

# Shared helpers
from .common import (
    build_async_executor,
    build_sync_executor,
    build_tool_error_payload,
    to_tool_call_view,
    create_args_schema,
    enhance_description,
    is_nullable,
    tool_input_schema,
    tool_instance_id,
    tool_name,
)

class OpenAIAdapter:
    """
    Adapter that converts MCPStore tools into the OpenAI function calling format.

    Compatible with langchain-openai's bind_tools and the direct OpenAI API.
    """

    def __init__(self, context: Any, instance_id: str | None = None):
        self._context = context
        self._instance_id = instance_id

    def list_tools(self) -> List[Dict[str, Any]]:
        """List all MCPStore tools and convert them to the OpenAI function format (sync version)."""
        if self._instance_id is None:
            tools = self._context.list_tools()
        else:
            tools = self._context.list_tools(self._instance_id)
        return [
            self._convert_to_openai_format(tool_info)
            for tool_info in tools
        ]


    def _convert_to_openai_format(self, tool_info: Any) -> Dict[str, Any]:
        """
        Convert MCPStore tool metadata into the OpenAI function calling format.

        OpenAI function format:
        {
            "type": "function",
            "function": {
                "name": "function_name",
                "description": "Function description",
                "parameters": {
                    "type": "object",
                    "properties": {...},
                    "required": [...]
                }
            }
        }
        """
        # Enhanced description
        enhanced_description = enhance_description(tool_info)

        # Read the input arguments schema
        input_schema = tool_input_schema(tool_info)
        properties = input_schema.get("properties", {})
        required = input_schema.get("required", [])

        # Convert the parameters schema to the OpenAI format
        openai_parameters = {
            "type": "object",
            "properties": {},
            "required": required
        }

        # Pass through the top-level additionalProperties
        if "additionalProperties" in input_schema:
            openai_parameters["additionalProperties"] = input_schema["additionalProperties"]

        def _process_schema(p: Dict[str, Any]) -> Dict[str, Any]:
            """Recursively process a JSON Schema node into an OpenAI-compatible format."""
            out: Dict[str, Any] = {}
            declared_type = p.get("type", "string")

            # Check nullability with the shared helper
            nullable = is_nullable(p)

            if nullable:
                base_type = (
                    declared_type
                    if isinstance(declared_type, str)
                    else next((t for t in declared_type if t != "null"), "string")
                )
                out["anyOf"] = [{"type": base_type}, {"type": "null"}]
            else:
                out["type"] = declared_type

            if "enum" in p:
                out["enum"] = p["enum"]
            if "default" in p:
                out["default"] = p["default"]

            # Array handling
            if (
                declared_type == "array" or (isinstance(declared_type, list) and "array" in declared_type)
            ) and "items" in p:
                out["items"] = _process_schema(p["items"]) if isinstance(p["items"], dict) else p["items"]
                for k in ("minItems", "maxItems", "uniqueItems"):
                    if k in p:
                        out[k] = p[k]

            # Object handling
            is_object_type = (
                declared_type == "object"
                or (isinstance(declared_type, list) and "object" in declared_type)
            )
            if is_object_type and "properties" in p:
                out["properties"] = {}
                for child_name, child_schema in p["properties"].items():
                    if isinstance(child_schema, dict):
                        out["properties"][child_name] = _process_schema(child_schema)
                    else:
                        out["properties"][child_name] = child_schema
                if "required" in p:
                    out["required"] = p["required"]
                if "additionalProperties" in p:
                    out["additionalProperties"] = p["additionalProperties"]

            return out

        # Process each parameter
        for param_name, param_info in properties.items():
            openai_param: Dict[str, Any] = {"description": param_info.get("description", "")}
            # Merge the processed schema (type/anyOf, enum/default, nested items/properties)
            openai_param.update(_process_schema(param_info))
            openai_parameters["properties"][param_name] = openai_param

        # When there are no parameters, build the empty parameter structure
        if not properties:
            openai_parameters = {
                "type": "object",
                "properties": {},
                "required": []
            }

        # Build the OpenAI function payload
        openai_tool = {
            "type": "function",
            "function": {
                "name": tool_name(tool_info),
                "description": enhanced_description,
                "parameters": openai_parameters
            }
        }

        return openai_tool

    def get_callable_tools(self) -> List[Dict[str, Any]]:
        """
        Get tools with callable executors attached.

        Returns:
            A list of dicts containing 'tool' (OpenAI format) and 'callable' (executor function).
        """
        callable_tools = []
        tools = self._context.list_tools() if self._instance_id is None else self._context.list_tools(self._instance_id)
        for tool_info in tools:
            openai_tool = self._convert_to_openai_format(tool_info)
            args_schema = create_args_schema(tool_info)
            name = tool_name(tool_info)
            instance_id = None if self._instance_id is None else tool_instance_id(tool_info)
            callable_tools.append(
                {
                    "tool": openai_tool,
                    "callable": build_sync_executor(
                        self._context,
                        instance_id,
                        name,
                        args_schema,
                    ),
                    "async_callable": build_async_executor(
                        self._context,
                        instance_id,
                        name,
                        args_schema,
                    ),
                    "name": name,
                    "schema": args_schema,
                }
            )
        return callable_tools


    def create_tool_registry(self) -> Dict[str, Any]:
        """
        Build a tool registry for executing tools by name.

        Returns:
            A dict mapping tool names to executors and metadata.
        """
        return self._registry_from_callable_tools(self.get_callable_tools())


    @staticmethod
    def _registry_from_callable_tools(callable_tools: List[Dict[str, Any]]) -> Dict[str, Any]:
        registry = {}
        for tool_data in callable_tools:
            registry[tool_data["name"]] = {
                "openai_format": tool_data["tool"],
                "execute": tool_data["callable"],
                "execute_async": tool_data["async_callable"],
                "schema": tool_data["schema"],
            }
        return registry

    def execute_tool_call(self, tool_call: Dict[str, Any]) -> str:
        """
        Execute a tool call from the OpenAI response format.

        Args:
            tool_call: The OpenAI tool call payload with 'name' and 'arguments'.

        Returns:
            str: The tool execution result.
        """
        tool_name = None
        try:
            tool_name, arguments = self._parse_tool_call(tool_call)
            if self._instance_id is None:
                result = self._context.call_tool(tool_name, arguments)
            else:
                result = self._context.call_tool(self._instance_id, tool_name, arguments)
            return self._format_tool_result(tool_name, arguments, result)
        except Exception as e:
            return f"Tool '{tool_name}' execution failed: {str(e)}"

    def batch_execute_tool_calls(self, tool_calls: List[Dict[str, Any]]) -> List[str]:
        """
        Execute multiple tool calls in batch.

        Args:
            tool_calls: A list of OpenAI tool call payloads.

        Returns:
            List[str]: The list of tool execution results.
        """
        results = []
        for tool_call in tool_calls:
            try:
                results.append(self.execute_tool_call(tool_call))
            except Exception as e:
                results.append(f"Error executing tool call: {str(e)}")
        return results


    @staticmethod
    def _parse_tool_call(tool_call: Dict[str, Any]) -> Tuple[str, Dict[str, Any]]:
        name = tool_call.get("name") or tool_call.get("function", {}).get("name")
        arguments = tool_call.get("arguments") or tool_call.get("function", {}).get("arguments", {})

        if not name:
            raise ValueError("Tool name not found in tool_call")

        if isinstance(arguments, str):
            try:
                arguments = json.loads(arguments)
            except json.JSONDecodeError:
                raise ValueError("Tool arguments JSON parse failed")

        if not isinstance(arguments, dict):
            raise ValueError("Tool arguments must be a dict")

        return name, arguments

    @staticmethod
    def _format_tool_result(name: str, arguments: Dict[str, Any], result: Any) -> str:
        view = to_tool_call_view(result)
        if view.is_error:
            payload = build_tool_error_payload(
                name,
                view.error_message or view.text or "Tool execution failed",
                tool_input=arguments,
                view=view,
            )
            return json.dumps(payload, ensure_ascii=False)
        actual_result = view.structured if view.structured is not None else view.data
        if actual_result is None:
            actual_result = view.text

        if isinstance(actual_result, (dict, list)):
            return json.dumps(actual_result, ensure_ascii=False)
        return str(actual_result)
