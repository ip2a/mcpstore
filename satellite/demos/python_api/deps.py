from __future__ import annotations

from typing import Any

from fastapi import Request


def get_store(request: Request) -> Any:
    return request.app.state.store
