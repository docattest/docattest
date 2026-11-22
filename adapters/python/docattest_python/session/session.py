"""The session ``open`` creates: one project, for one docattest invocation.

Static questions (``exports``, ``symbols``, ``compile``, ``tests``, ``inCode``,
``affected``, ``describeSurfaces``) are answered from syntax. Whatever has to
run project code (``evaluate``, ``run``, ``observables``) is sent to the worker pool;
``runTests`` runs the project's own test runner.

A session is made of parts that share one ``Workspace`` (the project's files, modules and
surfaces): ``problems`` answers ``compile``, ``symbols`` answers ``symbols`` and ``tests``,
and ``running`` sends work to the pool. ``continued``
remembers the examples the core has named, which the three need for blocks tagged
``continue``.
"""

from __future__ import annotations

import os
import tempfile

from .. import language
from ..analysis import impact
from ..analysis import surfaces as surface_rules
from ..workers.docattest_worker.jobs import Settings
from ..workers.interpreter import Interpreter, choose_interpreter
from ..workers.pool import WorkerPool
from ..workers.worker_process import WorkerSpec
from .cited_tests import CitedTests, OnDisk
from .continued import ContinuedBlocks
from .problems import Problems
from .running import Running
from .settings import SessionSettings
from .symbols import Symbols
from .workspace import Workspace


class Session:
    def __init__(self, settings: SessionSettings) -> None:
        self.temp: tempfile.TemporaryDirectory[str] | None = None
        root = settings.root if settings.files is None else self._materialise(settings.files)
        self.interpreter = Interpreter(choose_interpreter(root), root)
        self.workspace = Workspace(root, settings.surfaces, settings.setup, self.interpreter.file_of)
        self.continued = ContinuedBlocks()
        self.continued.remember(settings.examples)
        self.problems = Problems(self.workspace, self.continued, self.interpreter)
        self.symbols = Symbols(self.workspace, self.continued)
        spec = WorkerSpec(self.interpreter.python, root, self._worker_settings(settings))
        workers = settings.workers or os.cpu_count() or 1
        # Resolved, as pytest resolves the paths it names its tests by.
        on_disk = (
            OnDisk(os.path.realpath(root), self.workspace.project.search) if settings.files is None else None
        )
        self.cited_tests = CitedTests(on_disk, self.symbols, self.interpreter, workers)
        self.running = Running(
            self.workspace,
            self.continued,
            lambda: WorkerPool(workers, spec, settings.timeout_ms),
            settings,
        )

    def _materialise(self, files: dict[str, str]) -> str:
        """An in-memory project is written to a temporary directory: Python imports files."""
        self.temp = tempfile.TemporaryDirectory(prefix="docattest-python-")
        root = os.path.realpath(self.temp.name)
        for rel, text in files.items():
            path = os.path.join(root, rel)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8") as handle:
                handle.write(text)
        return root

    def _worker_settings(self, settings: SessionSettings) -> Settings:
        """What every worker is told before its first job."""
        sessions = any(language.is_session(e.lang, e.code) for e in settings.examples)
        return {
            "path": list(self.workspace.project.search),
            "aliases": {s.import_name: s.file for s in self.workspace.surfaces if s.alias and s.file},
            "snapshots": settings.snapshots,
            "manifests": settings.manifests,
            "texts": settings.texts,
            "matches": settings.matches,
            "manifest_fields": settings.manifest_fields,
            "preload": ["doctest"] if sessions else [],
            "no_fork": bool(os.environ.get("DOCATTEST_PYTHON_NO_FORK")),
        }

    def describe_surfaces(self) -> dict[str, dict[str, object]]:
        return {s.name: _describe(self.workspace, s) for s in self.workspace.surfaces}

    def affected(self, base: dict[str, str | None], changed: list[str]) -> list[dict[str, str]]:
        workspace = self.workspace
        tree = impact.WorkingTree(
            workspace.root, workspace.read, workspace.configured, workspace.project.search
        )
        return impact.affected(tree, base, changed)

    def close(self) -> None:
        self.running.close()
        self.cited_tests.close()
        if self.temp is not None:
            self.temp.cleanup()


def _describe(workspace: Workspace, s: surface_rules.Surface) -> dict[str, object]:
    """Where one surface resolved, and what the report should warn about it."""
    if s.file is None:
        where = f" ({s.written} does not exist)" if s.written is not None else ""
        return {
            "warnings": [f'"{s.import_name}" did not resolve{where}; examples and facts that use it fail']
        }
    resolution: dict[str, object] = {"import": s.import_name, "entry": workspace.rel(s.file)}
    inside = not os.path.relpath(s.file, workspace.root).startswith("..")
    package = (
        surface_rules.nearest_package(workspace.project, workspace.root, s.file)
        if inside
        else surface_rules.installed_package(s.file)
    )
    if package:
        resolution["package"] = package.to_json()
    warnings: list[str] = []
    if s.module is None:
        warnings.append(f"{workspace.rel(s.file)} is not Python source; its exports cannot be read")
    elif s.module.error:
        warnings.append(
            f"{workspace.rel(s.file)} does not parse ({s.module.error}); its exports cannot be read"
        )
    elif s.module.all_dynamic:
        warnings.append(
            f"__all__ of {s.import_name} is computed when it is imported; "
            "docattest read the module's public names instead"
        )
    return {"resolution": resolution, "warnings": warnings}
