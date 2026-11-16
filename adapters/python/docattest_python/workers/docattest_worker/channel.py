"""The channel: where the worker's messages go, and where attested code's prints go."""

from __future__ import annotations

import json
import os
from collections.abc import Iterable

from .messages import Message


class Channel:
    def __init__(self, fd: int) -> None:
        self.fd = fd

    def post(self, message: Message) -> None:
        data = (json.dumps(message) + "\n").encode("utf-8")
        while data:
            written = os.write(self.fd, data)
            data = data[written:]


class Sink:
    """What ``print`` writes to in attested code: nowhere."""

    encoding = "utf-8"
    errors = "replace"

    def write(self, text: str) -> int:
        return len(text)

    def writelines(self, lines: Iterable[str]) -> None:
        pass

    def flush(self) -> None:
        pass

    def isatty(self) -> bool:
        return False

    def fileno(self) -> int:
        return 1  # which is stderr here
