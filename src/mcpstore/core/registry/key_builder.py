from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class KeyBuilder:
    """Scope-aware key builder for namespacing cache keys.

    Key layout (initial version):
      mcpstore:{namespace}:{dataspace}:scope:{scope}:{owner}:{entity}:{identifier}
    where scope in {device, user, pub}.
    """

    namespace: str = "default"
    dataspace: str = "default"

    def base(self) -> str:
        # Global project prefix per design: 'mcpstore'
        return f"mcpstore:{self.namespace}:{self.dataspace}"

    def scope(self, scope: str, owner: str) -> str:
        return f"{self.base()}:scope:{scope}:{owner}"

    def entity(self, scope: str, owner: str, entity: str, identifier: str) -> str:
        return f"{self.scope(scope, owner)}:{entity}:{identifier}"

