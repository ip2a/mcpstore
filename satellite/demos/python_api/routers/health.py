from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Depends

from deps import get_store


router = APIRouter()


@router.get("/health")
def health(store: Any = Depends(get_store)) -> dict[str, Any]:
    return {"status": "ok", "store": store.current_store()}
