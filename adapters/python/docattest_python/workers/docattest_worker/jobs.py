"""What the adapter sends the worker: the settings, once, and then one request per job.

Each is a JSON line on the worker's stdin. The adapter builds these types and the worker
reads them back with ``read_settings`` and ``read_request``, so a job that lacks a field
stops the worker at once instead of running with a default.
"""

from __future__ import annotations

from typing import Literal, TypedDict

from .fields import Fields, MalformedMessage


class Snapshots(TypedDict):
    #: The snapshots directory, for the message when a name is not found.
    dir: str | None
    data: dict[str, object]


class Settings(TypedDict):
    """What every job of one session shares."""

    #: Directories put first on ``sys.path``.
    path: list[str]
    #: Import name → file, for surfaces imported by another name than their file's.
    aliases: dict[str, str]
    snapshots: Snapshots
    #: The data of each manifest the facts name, by the path as they write it.
    manifests: dict[str, object]
    #: Modules imported once, before any job is forked.
    preload: list[str]
    #: Run every job in a fresh process, even where ``os.fork`` exists.
    no_fork: bool


class ContinuedBlock(TypedDict):
    """A block that the job's block goes on from (``continue``)."""

    file: str
    line: int
    code: str
    session: bool


class BlockJob(TypedDict):
    """One block: a script (``example``) or an interpreter session."""

    type: Literal["example", "session"]
    file: str
    code: str
    weak: bool
    network: bool
    before: list[ContinuedBlock]
    #: For a docstring's session: the module whose names it starts with.
    module: str | None


class SurfaceToLoad(TypedDict):
    name: str
    import_name: str
    #: What the surface exports, as read from its syntax; None when it could not be read.
    exports: list[str] | None


class SetupModule(TypedDict):
    file: str
    #: The path as the config wrote it, for messages.
    label: str


class FactToEvaluate(TypedDict):
    expression: str
    #: The names the expression reads and does not bind.
    names: list[str]


class FactsJob(TypedDict):
    type: Literal["facts"]
    surface: SurfaceToLoad
    setup: SetupModule | None
    facts: list[FactToEvaluate]


class CasesRow(TypedDict):
    line: int
    name: str
    values: dict[str, str]


class CasesJob(TypedDict):
    type: Literal["cases"]
    setup: SetupModule
    runner: str
    page: str
    line: int
    header: list[str]
    rows: list[CasesRow]


class ObservablesJob(TypedDict):
    type: Literal["observables"]
    surface: SurfaceToLoad
    #: The setup module whose named facts are offered too: only on the default surface.
    setup: SetupModule | None


Job = BlockJob | FactsJob | CasesJob | ObservablesJob


class JobRequest(TypedDict):
    job: Job
    #: The item to start from: a job that lost its worker goes on from where it stopped.
    start: int


def read_settings(value: object) -> Settings:
    fields = Fields(value, "the worker's settings")
    snapshots = fields.object("snapshots")
    return {
        "path": fields.strings("path"),
        "aliases": fields.string_map("aliases"),
        "snapshots": {"dir": snapshots.optional_string("dir"), "data": snapshots.json_object("data")},
        "manifests": fields.json_object("manifests"),
        "preload": fields.strings("preload"),
        "no_fork": fields.boolean("no_fork"),
    }


def read_request(value: object) -> JobRequest:
    fields = Fields(value, "a job request")
    return {"job": _read_job(fields.object("job")), "start": fields.integer("start")}


def _read_job(fields: Fields) -> Job:
    kind = fields.string("type")
    if kind == "example" or kind == "session":
        return _read_block(fields, kind)
    if kind == "facts":
        return {
            "type": "facts",
            "surface": _read_surface(fields.object("surface")),
            "setup": None if fields.is_null("setup") else _read_setup(fields.object("setup")),
            "facts": [
                {"expression": f.string("expression"), "names": f.strings("names")}
                for f in fields.objects("facts")
            ],
        }
    if kind == "cases":
        return _read_cases(fields)
    if kind == "observables":
        return {
            "type": "observables",
            "surface": _read_surface(fields.object("surface")),
            "setup": None if fields.is_null("setup") else _read_setup(fields.object("setup")),
        }
    raise MalformedMessage(f'{fields.where}: unknown job type "{kind}"')


def _read_block(fields: Fields, kind: Literal["example", "session"]) -> BlockJob:
    return {
        "type": kind,
        "file": fields.string("file"),
        "code": fields.string("code"),
        "weak": fields.boolean("weak"),
        "network": fields.boolean("network"),
        "before": [
            {
                "file": b.string("file"),
                "line": b.integer("line"),
                "code": b.string("code"),
                "session": b.boolean("session"),
            }
            for b in fields.objects("before")
        ],
        "module": fields.optional_string("module"),
    }


def _read_cases(fields: Fields) -> CasesJob:
    return {
        "type": "cases",
        "setup": _read_setup(fields.object("setup")),
        "runner": fields.string("runner"),
        "page": fields.string("page"),
        "line": fields.integer("line"),
        "header": fields.strings("header"),
        "rows": [
            {"line": r.integer("line"), "name": r.string("name"), "values": r.string_map("values")}
            for r in fields.objects("rows")
        ],
    }


def _read_surface(fields: Fields) -> SurfaceToLoad:
    exports = None if fields.is_null("exports") else fields.strings("exports")
    return {"name": fields.string("name"), "import_name": fields.string("import_name"), "exports": exports}


def _read_setup(fields: Fields) -> SetupModule:
    return {"file": fields.string("file"), "label": fields.string("label")}
