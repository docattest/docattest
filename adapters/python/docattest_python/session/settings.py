"""What ``open`` says about the session, as one typed value."""

from __future__ import annotations

from dataclasses import dataclass

from ..analysis.surfaces import ConfiguredSurface
from ..attestations import Example
from ..workers.docattest_worker.jobs import Snapshots


@dataclass(frozen=True)
class SessionSettings:
    #: The project's directory, absolute.
    root: str
    #: Only for a project that exists in memory: every file of it, by path.
    files: dict[str, str] | None
    surfaces: list[ConfiguredSurface]
    #: The setup module, relative to the root.
    setup: str | None
    timeout_ms: float
    #: How many blocks run at once; 0 for one per CPU.
    workers: int
    network: bool
    snapshots: Snapshots
    #: The data of each manifest the facts name, by the path as they write it.
    manifests: dict[str, object]
    #: The text of each file the facts read with ``file``, by the path as they write it.
    texts: dict[str, str]
    #: Every example the session may be asked about, in page order.
    examples: list[Example]
