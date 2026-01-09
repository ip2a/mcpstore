from typing import Optional, Any

ERROR_PREFIX = "[MCPSTORE_ERROR]"


def raise_legacy_error(feature: str, detail: Optional[str] = None) -> None:
    message = f"{ERROR_PREFIX} {feature} is disabled."
    if detail:
        message = f"{message} {detail}"
    raise RuntimeError(message)


class LegacyManagerProxy:
    def __init__(self, name: str, detail: Optional[str] = None):
        self._name = name
        self._detail = detail

    def __getattr__(self, attr: str) -> Any:
        raise_legacy_error(f"{self._name}.{attr}", self._detail)
