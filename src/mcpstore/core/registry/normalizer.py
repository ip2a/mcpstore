from __future__ import annotations

from typing import Protocol, Dict, Any


class ToolNormalizer(Protocol):
    """Normalizes tool definitions for backend storage (JSON-compatible).

    Implementations should:
    - Remove non-serializable objects
    - Keep stable fields: name, description, parameters, service_name, etc.
    - Optionally compact or canonicalize ordering
    """

    def normalize_tool(self, tool_name: str, tool_def: Dict[str, Any]) -> Dict[str, Any]:
        ...


class DefaultToolNormalizer:
    def normalize_tool(self, tool_name: str, tool_def: Dict[str, Any]) -> Dict[str, Any]:
        # Shallow best-effort normalization for skeleton stage
        out: Dict[str, Any] = {}
        if isinstance(tool_def, dict):
            if "function" in tool_def and isinstance(tool_def["function"], dict):
                fn = tool_def["function"]
                out["type"] = "function"
                out_fn = {
                    "name": fn.get("name", tool_name),
                    "description": fn.get("description", ""),
                    "service_name": fn.get("service_name", ""),
                    "parameters": fn.get("parameters"),
                }
                out["function"] = out_fn
            else:
                # Fallback mapping
                out.update({
                    "name": tool_def.get("name", tool_name),
                    "description": tool_def.get("description", ""),
                    "service_name": tool_def.get("service_name", ""),
                    "parameters": tool_def.get("parameters"),
                })
        else:
            out = {"name": tool_name, "description": str(tool_def)}
        return out

