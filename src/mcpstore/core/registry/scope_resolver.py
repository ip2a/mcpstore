from __future__ import annotations

from dataclasses import dataclass
from typing import List, Tuple, Optional


@dataclass
class ScopeContext:
    device_id: Optional[str] = None
    user_id: Optional[str] = None


class ScopeResolver:
    """Computes read precedence across scopes: device > user > pub.

    - For read operations, return the ordered list of (scope, owner) to check.
    - For write operations, callers decide the target scope; usually device or user.
    """

    PUB_OWNER = "_pub"

    def get_read_order(self, ctx: ScopeContext) -> List[Tuple[str, str]]:
        order: List[Tuple[str, str]] = []
        if ctx.device_id:
            order.append(("device", ctx.device_id))
        if ctx.user_id:
            order.append(("user", ctx.user_id))
        order.append(("pub", self.PUB_OWNER))
        return order

