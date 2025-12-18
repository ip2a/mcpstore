from typing import Optional

ERROR_PREFIX = "[MCPSTORE_ERROR]"


def raise_legacy_error(feature: str, detail: Optional[str] = None) -> None:
    message = f"{ERROR_PREFIX} {feature} is disabled."
    if detail:
        message = f"{message} {detail}"
    raise RuntimeError(message)
