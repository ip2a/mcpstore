"""Validation helpers for Rust-backed API routes."""

from __future__ import annotations

import re

from mcpstore.core.models import ErrorCode


_AGENT_ID_PATTERN = re.compile(r"^[A-Za-z0-9_.:-]+$")


def validate_agent_id(agent_id: str) -> str:
    if not agent_id or not _AGENT_ID_PATTERN.match(agent_id):
        try:
            from fastapi import HTTPException

            raise HTTPException(
                status_code=400,
                detail={
                    "code": ErrorCode.INVALID_PARAMETER.value,
                    "message": "Invalid agent_id",
                    "field": "agent_id",
                },
            )
        except ImportError:
            raise ValueError("Invalid agent_id")
    return agent_id
