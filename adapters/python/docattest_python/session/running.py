"""Whatever has to run project code (``evaluate``, ``run``, ``observables``): jobs sent to
the worker pool, and the results they come back as."""

from __future__ import annotations

import ast
import threading
from collections.abc import Callable
from concurrent.futures import Future
from typing import TypeVar

from .. import language
from ..analysis.constants import literal
from ..analysis.names import expression_names
from ..analysis.surfaces import Surface
from ..attestations import Cases, Example, Fact
from ..errors import UserError
from ..workers.docattest_worker.jobs import (
    BlockJob,
    CasesJob,
    ContinuedBlock,
    FactsJob,
    FactToEvaluate,
    ObservablesJob,
    SetupModule,
    SurfaceToLoad,
)
from ..workers.docattest_worker.messages import Evaluation, Observable, Verdict
from ..workers.pool import ItemResult, JobResult, Lost, TimedOut, WorkerPool
from ..workers.worker_process import ms
from .continued import ContinuedBlocks
from .settings import SessionSettings
from .workspace import Workspace

#: The surface a fact without ``@`` reads (PROTOCOL.md, ``observables``).
DEFAULT_SURFACE = "default"
NETWORK_SKIPPED = "network example skipped: the network is off (set DOCATTEST_NETWORK=1 to run it)"

T = TypeVar("T")
R = TypeVar("R")


class Running:
    """What sends work to the pool. The pool and its workers start with the first job."""

    def __init__(
        self,
        workspace: Workspace,
        continued: ContinuedBlocks,
        start_pool: Callable[[], WorkerPool],
        settings: SessionSettings,
    ) -> None:
        self.workspace = workspace
        self.continued = continued
        self.network = settings.network
        self.timeout_ms = settings.timeout_ms
        self._start_pool = start_pool
        self._pool: WorkerPool | None = None
        self._lock = threading.Lock()

    def pool(self) -> WorkerPool:
        with self._lock:
            if self._pool is None:
                self._pool = self._start_pool()
            return self._pool

    def close(self) -> None:
        if self._pool is not None:
            self._pool.close()

    # --- evaluate ------------------------------------------------------------------------

    def evaluate(self, facts: list[Fact]) -> Future[dict[str, Evaluation]]:
        values: dict[str, Evaluation] = {}
        groups: dict[tuple[str, str], list[Fact]] = {}
        for fact in facts:
            problem = self._fact_problem(fact)
            if problem:
                values[fact.id] = {"ok": False, "message": problem}
            else:
                groups.setdefault((fact.page, fact.surface), []).append(fact)
        futures = [self._submit_facts(group) for group in groups.values()]
        ordered = list(groups.values())

        def combine(results: list[JobResult]) -> dict[str, Evaluation]:
            for group, result in zip(ordered, results):
                values.update(self._evaluations(group, result))
            return values

        return gather(futures, combine)

    def _submit_facts(self, group: list[Fact]) -> Future[JobResult]:
        """The facts of one page about one surface, as one job."""
        surface = self.workspace.by_name.get(group[0].surface)
        if surface is None:
            facts: list[FactToEvaluate] = [
                {"expression": f.expression, "names": expression_names(f.expression), "imports": False}
                for f in group
            ]
            empty_surface: SurfaceToLoad = {
                "name": group[0].surface,
                "import_name": "",
                "exports": [],
                "constants": {},
            }
            job: FactsJob = {
                "type": "facts",
                "surface": empty_surface,
                "setup": self._setup_job(),
                "facts": facts,
                "refused": {},
            }
            return self.pool().submit(job, len(group))
        reading = self.workspace.readable(surface.name)
        facts = []
        for f in group:
            names = expression_names(f.expression)
            facts.append({"expression": f.expression, "names": names, "imports": reading.imports(names)})
        read = {name for fact in facts for name in fact["names"]}
        job = {
            "type": "facts",
            "surface": self._surface_job(surface, read),
            "setup": self._setup_job(),
            "facts": facts,
            "refused": {name: why for name, why in reading.refused.items() if name in read},
        }
        return self.pool().submit(job, len(group))

    def _evaluations(self, group: list[Fact], result: JobResult) -> dict[str, Evaluation]:
        if result.failure:
            failed = result.failure.message
            if result.failure.timed_out:
                failed = f'surface "{group[0].surface}" failed to load: {failed}'
            return {fact.id: {"ok": False, "message": failed} for fact in group}
        return {
            fact.id: self._evaluation(fact, result.items[k] if k < len(result.items) else None)
            for k, fact in enumerate(group)
        }

    def _evaluation(self, fact: Fact, item: ItemResult | None) -> Evaluation:
        label = f"fact `{fact.expression}`"
        if item is None:
            return {"ok": False, "message": f"{label} was not evaluated"}
        if isinstance(item, TimedOut):
            return {"ok": False, "message": f"{label} {self._timed_out()}"}
        if isinstance(item, Lost):
            return {"ok": False, "message": f"{label}: {item.message}"}
        if item["t"] == "evaluation":
            return item["evaluation"]
        return {"ok": False, "message": f"{label}: the worker returned no evaluation"}

    def _fact_problem(self, fact: Fact) -> str | None:
        """Why a fact cannot be evaluated, found before anything runs; None when it can be."""
        if problem := self._surface_problem(fact):
            return problem
        try:
            ast.parse(f"(\n{fact.expression}\n)", mode="eval")
        except SyntaxError as err:
            return f"cannot parse fact expression `{fact.expression}`: {err.msg}"
        surface = self.workspace.by_name.get(fact.surface)
        return None if surface is None else self._helper_clash(surface.name, fact.expression)

    def _surface_problem(self, fact: Fact) -> str | None:
        if not self.workspace.surfaces and fact.surface == "default":
            return None
        surface = self.workspace.by_name.get(fact.surface)
        if surface is None:
            configured = ", ".join(s.name for s in self.workspace.surfaces)
            return f'unknown surface "{fact.surface}" (configured: {configured})'
        if surface.file is None:
            return f'surface "{surface.name}" ({surface.import_name}) not found'
        return None

    def _helper_clash(self, surface_name: str, expression: str) -> str | None:
        exported = set(self.workspace.exports(surface_name) or [])
        helpers = set(self.workspace.setup_entries("facts"))
        for name in expression_names(expression):
            if name in exported and name in helpers:
                return f'fact `{expression}`: "{name}" is both an export of surface "{surface_name}" and a setup fact; rename the setup fact'
        return None

    def _surface_job(self, surface: Surface, read: set[str]) -> SurfaceToLoad:
        """A surface as the worker loads it, with the constants among the names ``read``."""
        reading = self.workspace.readable(surface.name)
        return {
            "name": surface.name,
            "import_name": surface.import_name,
            "exports": self.workspace.exports(surface.name),
            "constants": {name: literal(value) for name, value in reading.constants.items() if name in read},
        }

    def _setup_job(self) -> SetupModule | None:
        setup = self.workspace.setup
        if setup is None or self.workspace.read(self.workspace.abs(setup)) is None:
            return None
        return {"file": self.workspace.abs(setup), "label": setup}

    def _timed_out(self) -> str:
        return f"timed out after {ms(self.timeout_ms)} ms"

    # --- run -----------------------------------------------------------------------------

    def run(self, examples: list[Example], cases: list[Cases]) -> Future[list[dict[str, object]]]:
        futures = [self._run_example(e) for e in examples] + [self._run_cases(c) for c in cases]
        return gather(futures, lambda lists: [r for results in lists for r in results])

    def _run_example(self, e: Example) -> Future[list[dict[str, object]]]:
        before = self.continued.before(e) or []
        if not self.network and (e.network or any(x.network for x in before)):
            return resolved([_result(e, "skipped", NETWORK_SKIPPED)])
        job = self._example_job(e, before)
        return gather([self.pool().submit(job, 1)], lambda results: self._example_verdict(e, results[0]))

    def _example_job(self, e: Example, before: list[Example]) -> BlockJob:
        """What the worker is sent to run a block, with the blocks it continues."""
        session = language.is_session(e.lang, e.code)
        module = None
        if session and language.is_source(e.page):
            # A docstring's session starts with its module's names, as doctest gives them.
            name, base = self.workspace.project.module_name(self.workspace.abs(e.page))
            if name and base in self.workspace.project.search:
                module = name
        return {
            "type": "session" if session else "example",
            "file": f"<{e.page} L{e.line}>",
            "code": e.code,
            "weak": e.weak,
            "network": e.network or any(x.network for x in before),
            "before": [_continued(x) for x in before],
            "module": module,
        }

    def _example_verdict(self, e: Example, result: JobResult) -> list[dict[str, object]]:
        item = result.items[0] if result.items else None
        if result.failure:
            v: Verdict = {"status": "fail", "message": result.failure.message}
        elif item is None:
            v = {"status": "fail", "message": "the block produced no result"}
        else:
            v = self._verdict(item)
        return [_result(e, v["status"], v["message"], v)]

    def _verdict(self, item: ItemResult, name: str | None = None) -> Verdict:
        prefix = "" if name is None else f"{name}: "
        if isinstance(item, TimedOut):
            return {"status": "fail", "message": f"{prefix}{self._timed_out()}"}
        if isinstance(item, Lost):
            return {"status": "fail", "message": f"{prefix}{item.message}"}
        if item["t"] == "verdict":
            return item["verdict"]
        return {"status": "fail", "message": f"{prefix}the worker returned no verdict"}

    def _run_cases(self, c: Cases) -> Future[list[dict[str, object]]]:
        at = f'cases "{c.runner}" at L{c.line}'
        if self.workspace.setup is None:
            message = f'{at}: no setup module is configured ("setup" in docattest.config.json)'
            return resolved([_result(c, "error", message)])
        setup = self._setup_job()
        if setup is None:
            return resolved([_result(c, "error", f"{at}: setup module {self.workspace.setup} not found")])
        if not c.rows:
            return resolved([])
        job: CasesJob = {
            "type": "cases",
            "setup": setup,
            "runner": c.runner,
            "page": c.page,
            "line": c.line,
            "header": list(c.header),
            "rows": [{"line": r.line, "name": r.name, "values": r.values} for r in c.rows],
        }
        return gather([self.pool().submit(job, len(c.rows))], lambda results: self._rows(c, at, results[0]))

    def _rows(self, c: Cases, at: str, result: JobResult) -> list[dict[str, object]]:
        """The results of a table's rows."""
        if result.failure:
            message = result.failure.message
            if result.failure.timed_out:
                message = f"{at}: setup module {self.workspace.setup} failed to load: {message}"
            return [_result(c, "error", message)]
        out: list[dict[str, object]] = []
        for k, row in enumerate(c.rows):
            name = f"{c.page}:{row.line} {row.name}"
            item = result.items[k] if k < len(result.items) else None
            v: Verdict = (
                self._verdict(item, name) if item else {"status": "fail", "message": f"{name}: did not run"}
            )
            out.append(_result(c, v["status"], v["message"], v, line=row.line))
        return out

    # --- observables ---------------------------------------------------------------------

    def observables(self, name: str) -> Future[list[Observable]]:
        surface = self.workspace.by_name.get(name)
        if surface is None or surface.file is None:
            what = (
                f'surface "{name}" ({surface.import_name}) not found'
                if surface
                else f'unknown surface "{name}"'
            )
            failed: Future[list[Observable]] = Future()
            failed.set_exception(UserError(what))
            return failed
        # The setup module's named facts are read with the default surface, as a fact
        # without `@` reads them.
        setup = self._setup_job() if name == DEFAULT_SURFACE else None
        # A surface is imported even when its exports are all constants, so that one that
        # cannot be imported is reported; a program is never imported, since that runs it.
        exports = self.workspace.exports(name) or []
        job: ObservablesJob = {
            "type": "observables",
            "surface": self._surface_job(surface, set(exports)),
            "setup": setup,
            "imports": not surface.is_program,
        }
        future = self.pool().submit(job, 1)
        return gather([future], lambda results: _observables(name, results[0]))


def _observables(name: str, result: JobResult) -> list[Observable]:
    """What a surface offers, or why it could not be read."""
    if result.failure:
        message = result.failure.message
        if result.failure.timed_out:
            message = f'surface "{name}" failed to load: {message}'
        raise UserError(message)
    item = result.items[0] if result.items else None
    if isinstance(item, dict) and item["t"] == "observables":
        return item["observables"]
    if isinstance(item, TimedOut):
        why = "timed out"
    else:
        why = item.message if isinstance(item, Lost) else "no answer"
    raise UserError(f'surface "{name}" could not be read: {why}')


def _result(
    attestation: Example | Cases,
    status: str,
    message: str,
    sides: Verdict | None = None,
    line: int | None = None,
) -> dict[str, object]:
    """A ``Result`` of the protocol: the verdict on an example or on a row of a table."""
    result: dict[str, object] = {
        "page": attestation.page,
        "line": line or attestation.line,
        "kind": attestation.kind,
        "status": status,
        "symbols": [],
        "message": message,
    }
    if sides is not None:
        if "expected" in sides:
            result["expected"] = sides["expected"]
        if "actual" in sides:
            result["actual"] = sides["actual"]
    return result


def _continued(x: Example) -> ContinuedBlock:
    """One of the blocks a block goes on from (``continue``), as the worker is sent it."""
    return {
        "file": f"<{x.page} L{x.line}>",
        "line": x.line,
        "code": x.code,
        "session": language.is_session(x.lang, x.code),
    }


def resolved(value: T) -> Future[T]:
    """A future that is already done."""
    done: Future[T] = Future()
    done.set_result(value)
    return done


def gather(futures: list[Future[T]], combine: Callable[[list[T]], R]) -> Future[R]:
    """A future for ``combine(results)`` once every future is done."""
    if not futures:
        return resolved(combine([]))
    out: Future[R] = Future()
    lock = threading.Lock()
    left = [len(futures)]

    def done(_: Future[T]) -> None:
        with lock:
            left[0] -= 1
            if left[0]:
                return
        try:
            out.set_result(combine([f.result() for f in futures]))
        except BaseException as err:  # noqa: BLE001 - the request waiting on it must hear
            out.set_exception(err)

    for f in futures:
        f.add_done_callback(done)
    return out
