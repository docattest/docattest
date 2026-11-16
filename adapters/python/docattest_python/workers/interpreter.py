"""The interpreter attested code runs on, and what it can import.

The adapter only reads syntax, but whether ``import x`` finds a module that is not the
project's own depends on the interpreter the code will run on, which may not be the one
the adapter runs on. That one is asked, in a process of its own, without importing
anything.
"""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
from typing import cast

from ..errors import UserError

#: How long the interpreter has to say which modules it finds, in seconds.
ASK_TIMEOUT = 30

# Run on the interpreter attested code runs on: for each name, where ``import`` finds it
# (its file; true for a module without one), or null.
_FIND_SPECS = (
    "import importlib.util, json, sys\n"
    "out = {}\n"
    "for n in sys.argv[1:]:\n"
    "    try:\n        s = importlib.util.find_spec(n)\n"
    "    except Exception:\n        s = None\n"
    "    out[n] = (s.origin or True) if s is not None else None\n"
    "print(json.dumps(out))\n"
)


def choose_interpreter(root: str) -> str:
    """``DOCATTEST_PYTHON``, else an active virtualenv, else the project's ``.venv``, else
    the interpreter the adapter itself runs on."""
    chosen = os.environ.get("DOCATTEST_PYTHON")
    if chosen:
        return chosen
    bin_dir, exe = ("Scripts", "python.exe") if os.name == "nt" else ("bin", "python")
    for venv in (os.environ.get("VIRTUAL_ENV"), os.path.join(root, ".venv")):
        candidate = os.path.join(venv, bin_dir, exe) if venv else None
        if candidate and os.path.isfile(candidate):
            same = os.path.realpath(os.path.dirname(candidate)) == os.path.realpath(
                os.path.dirname(sys.executable)
            )
            return sys.executable if same else candidate
    return sys.executable


class Interpreter:
    """What ``import`` finds on one interpreter, run from the project root."""

    def __init__(self, python: str, root: str) -> None:
        self.python = python
        self.root = root

    def is_own(self) -> bool:
        """Whether it is the interpreter the adapter runs on, which can be asked directly."""
        return self.python == sys.executable

    def finds(self, top: str) -> bool:
        """Whether ``import top`` finds a module (``top`` has no dots)."""
        if not self.is_own():
            return self._find_specs([top])[top] is not None
        try:
            return importlib.util.find_spec(top) is not None
        except (ImportError, ValueError, AttributeError):
            return False

    def file_of(self, name: str) -> str | None:
        """The file of a module that is not the project's own, found without importing it:
        only a top-level name is looked up (finding ``a.b`` would import ``a``)."""
        top = name.split(".")[0]
        origin = self._origin(top)
        if origin is None or "." not in name:
            return origin
        directory = os.path.dirname(origin)
        parts = name.split(".")[1:]
        for candidate in (
            os.path.join(directory, *parts, "__init__.py"),
            os.path.join(directory, *parts) + ".py",
        ):
            if os.path.isfile(candidate):
                return candidate
        return None

    def _origin(self, top: str) -> str | None:
        """The file a top-level module is loaded from, when it has one."""
        if self.is_own():
            try:
                spec = importlib.util.find_spec(top)
            except (ImportError, ValueError, AttributeError):
                return None
            origin = spec.origin if spec is not None else None
        else:
            found = self._find_specs([top])[top]
            origin = found if isinstance(found, str) else None
        return origin if origin and os.path.isfile(origin) else None

    def _find_specs(self, names: list[str]) -> dict[str, str | bool | None]:
        """Ask the interpreter where it finds each name. When it cannot be asked, nothing
        about what attested code can import is known, and that is the project's mistake."""
        try:
            done = subprocess.run(  # noqa: S603
                [self.python, "-c", _FIND_SPECS, *names],
                capture_output=True,
                text=True,
                timeout=ASK_TIMEOUT,
                cwd=self.root,
            )
        except (OSError, subprocess.TimeoutExpired) as err:
            raise self._cannot_ask(str(err)) from err
        if done.returncode != 0:
            raise self._cannot_ask(f"it exited with code {done.returncode}: {done.stderr.strip()}")
        return self._parse(done.stdout, names)

    def _parse(self, stdout: str, names: list[str]) -> dict[str, str | bool | None]:
        lines = stdout.strip().splitlines()
        try:
            found: object = json.loads(lines[-1]) if lines else None
        except ValueError:
            found = None
        if not isinstance(found, dict):
            raise self._cannot_ask(f"it answered {stdout.strip()[:200]!r}")
        answers: dict[str, str | bool | None] = {}
        for name in names:
            value = cast("dict[str, object]", found).get(name)
            answers[name] = value if isinstance(value, (str, bool)) else None
        return answers

    def _cannot_ask(self, why: str) -> UserError:
        return UserError(
            f"cannot ask the Python interpreter {self.python} which modules it can import: {why}"
        )
