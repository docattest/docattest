"""The two kinds of error a response can carry (PROTOCOL.md, section 1)."""


class UserError(Exception):
    """A mistake in the project. The core prints the message as given and adds nothing,
    so the ``docattest: `` prefix is the adapter's to write."""

    def __init__(self, message: str) -> None:
        super().__init__(message if message.startswith("docattest: ") else f"docattest: {message}")


class ProtocolError(Exception):
    """A mistake by the core: reported as ``internal``, without a traceback."""
