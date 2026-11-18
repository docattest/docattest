"""The setup module of this package's own guide (README.md, checked by docattest.config.json
beside it): the named facts its proofs read and the runners of its cases tables."""

from __future__ import annotations

import json
import os
import sys
import tempfile
from collections.abc import Callable
from typing import cast
from unittest import mock

from docattest.cells import code_spans
from docattest.launcher import cache_directory
from docattest_python.workers.interpreter import choose_interpreter

Row = dict[str, str]

_HERE = os.path.dirname(os.path.abspath(__file__))
_SYSTEMS = {"macOS": "darwin", "Linux": "linux", "Windows": "win32"}
_ANY_SYSTEM = "any system"
_HOME = "~"


def platforms() -> dict[str, dict[str, object]]:
    """The platforms a core is built for, from the package's copy of the protocol data."""
    path = os.path.join(_HERE, "docattest_python", "protocol", "platforms.json")
    with open(path, encoding="utf-8") as file:
        return cast("dict[str, dict[str, object]]", json.load(file)["platforms"])


facts = {"platforms": platforms}


def wheel_tags(row: Row) -> None:
    """A platform's wheel tags are the ones its wheel is built with."""
    built = platforms()[code_spans(row["name"])[0]]["wheel"]
    assert built == code_spans(row["wheel tags"])


def cache(row: Row) -> None:
    """The cache directory on one system, or on each, with the variables a row sets."""
    env = dict(span.split("=", 1) for span in code_spans(row["set"]))
    systems = _SYSTEMS.values() if row["on"] == _ANY_SYSTEM else [_SYSTEMS[row["on"]]]
    got = {system: cache_directory(env, system, _HOME) for system in systems}
    assert got == dict.fromkeys(systems, code_spans(row["cache"])[0])


def interpreter(row: Row) -> None:
    """Which interpreter runs attested code, given what is set and what exists."""
    with tempfile.TemporaryDirectory() as root:
        active = os.path.join(root, "active")
        named = {
            "`DOCATTEST_PYTHON`": "/named/python",
            "the active virtualenv": _fake_venv(active),
            "the project's `.venv`": "",
            "the adapter's own interpreter": sys.executable,
        }
        env = {"DOCATTEST_PYTHON": named["`DOCATTEST_PYTHON`"]} if row["DOCATTEST_PYTHON"] == "set" else {}
        if row["virtualenv"] == "active":
            env["VIRTUAL_ENV"] = active
        if row[".venv"] == "exists":
            named["the project's `.venv`"] = _fake_venv(os.path.join(root, ".venv"))
        with mock.patch.dict(os.environ, env, clear=True):
            got = choose_interpreter(root)
        assert got == named[row["runs on"]]


def _fake_venv(venv: str) -> str:
    """A virtualenv's interpreter as ``choose_interpreter`` finds it: only the file is needed."""
    bin_dir, exe = ("Scripts", "python.exe") if os.name == "nt" else ("bin", "python")
    python = os.path.join(venv, bin_dir, exe)
    os.makedirs(os.path.dirname(python))
    open(python, "w").close()
    return python


def _with_columns(runner: Callable[[Row], None], columns: list[str]) -> Callable[[Row], None]:
    runner.columns = columns  # type: ignore[attr-defined]
    return runner


cases = {
    "wheel_tags": _with_columns(wheel_tags, ["name", "wheel tags"]),
    "cache": _with_columns(cache, ["on", "set", "cache"]),
    "interpreter": _with_columns(interpreter, ["DOCATTEST_PYTHON", "virtualenv", ".venv", "runs on"]),
}
