"""Rust-backed compatibility helpers for historical session management."""

from __future__ import annotations

from typing import Optional


class SessionManagementMixin:
    """Compatibility mixin delegating session lifecycle to Rust core."""

    def create_session(self, session_id: str, user_session_id: Optional[str] = None):
        metadata = {"user_session_id": user_session_id} if user_session_id else None
        return self._rust_context().create_session(session_id, metadata=metadata)

    def find_session(self, session_id: Optional[str] = None, is_user_session_id: bool = False):
        if is_user_session_id:
            return self._rust_context().find_user_session(session_id)
        return self._rust_context().find_session(session_id)

    def find_user_session(self, user_session_id: str):
        return self.find_session(user_session_id, is_user_session_id=True)

    def find_session_by_key(self, session_key: str):
        return self._rust_context().find_session_by_key(session_key)

    def session_by_key(self, session_key: str):
        return self._rust_context().session_by_key(session_key)

    def get_session(self, session_id: str):
        return self._rust_context().get_session(session_id)

    def list_sessions(self):
        return self._rust_context().list_sessions()

    def session_auto(self, session_id: str = "auto_session_default", default_timeout: int = 720000, auto_cleanup: bool = False, session_prefix: str = "auto_"):
        return self._rust_context().session_auto(
            session_id=session_id,
            default_timeout=default_timeout,
            auto_cleanup=auto_cleanup,
            session_prefix=session_prefix,
        )

    def session_manual(self):
        return self._rust_context().session_manual()

    def is_session_auto(self) -> bool:
        return self._rust_context().is_session_auto()

    def current_session(self):
        return self._rust_context().current_session()

    def with_session(self, session_id: str):
        return self._rust_context().with_session(session_id)

    async def with_session_async(self, session_id: str):
        return self.with_session(session_id)

    def close_all_sessions(self):
        return self._rust_context().close_all_sessions()

    def cleanup_sessions(self):
        return self._rust_context().cleanup_sessions()

    def restart_sessions(self):
        return self._rust_context().restart_sessions()

    def create_shared_session(self, session_id: str, shared_id: str):
        return self.create_session(session_id, user_session_id=shared_id)

    def for_langchain_with_session(self, session_id: str, create_if_not_exists: bool = True):
        session = self.find_session(session_id)
        if session is None:
            if not create_if_not_exists:
                raise ValueError(f"Session not found: {session_id}")
            session = self.with_session(session_id)
        return session.for_langchain()

    def for_langchain_with_session_key(self, session_key: str):
        return self.session_by_key(session_key).for_langchain()

    def for_langchain_with_auto_session(self):
        session = self.current_session()
        if session is None:
            self.session_auto()
            session = self.current_session()
        return session.for_langchain()

    def for_langchain_with_shared_session(self, shared_id: str):
        session = self.find_user_session(shared_id)
        if session is None:
            session = self.create_shared_session(shared_id, shared_id)
        return session.for_langchain()

    def list_agent_sessions(self):
        return self.list_sessions()

    def get_session_statistics(self):
        sessions = self.list_sessions()
        active = [session for session in sessions if session.is_active]
        return {"total_sessions": len(sessions), "active_sessions": len(active), "sessions": sessions}

    def register_session_globally(self, session_id: str, global_id: str) -> bool:
        return self._rust_context().register_session_globally(session_id, global_id)

    def _rust_context(self):
        return getattr(self, "_context", self)


__all__ = ["SessionManagementMixin"]
