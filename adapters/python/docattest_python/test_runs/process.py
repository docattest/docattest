"""The edge that starts test runners: one process group per run, its output captured, killed
whole when it overruns its time or the session closes."""

from __future__ import annotations

import os
import signal
import subprocess
import threading
from dataclasses import dataclass

from ..errors import UserError


@dataclass(frozen=True)
class Finished:
    """A run that ended by itself: its exit code, and everything it wrote, both streams."""

    exit_code: int
    output: str


@dataclass(frozen=True)
class Overran:
    """A run that did not end within its time, and was killed."""


Ended = Finished | Overran


class RunnerProcesses:
    """The runner processes a session has started and not yet seen end."""

    def __init__(self) -> None:
        self._live: set[subprocess.Popen[bytes]] = set()
        self._lock = threading.Lock()
        self._closed = False

    def run(self, command: list[str], cwd: str, env: dict[str, str], timeout: float) -> Ended:
        """Run ``command`` to its end, or for ``timeout`` seconds. Nothing it writes reaches
        the adapter's stdout, which carries only responses."""
        process = self._start(command, cwd, env)
        try:
            output, _ = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            _kill(process)
            process.communicate()
            return Overran()
        finally:
            with self._lock:
                self._live.discard(process)
        return Finished(process.returncode, output.decode("utf-8", errors="replace"))

    def close(self) -> None:
        """Kill every runner still running; none starts after."""
        with self._lock:
            self._closed = True
            live = list(self._live)
        for process in live:
            _kill(process)

    def _start(self, command: list[str], cwd: str, env: dict[str, str]) -> subprocess.Popen[bytes]:
        with self._lock:
            if self._closed:
                raise UserError("the session was closed before the tests could run")
            try:
                process = subprocess.Popen(  # noqa: S603
                    command,
                    cwd=cwd,
                    env=env,
                    stdin=subprocess.DEVNULL,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.STDOUT,
                    start_new_session=os.name == "posix",
                )
            except OSError as err:
                # The interpreter is the project's choice, as it is for the examples.
                raise UserError(
                    f"cannot start the Python interpreter {command[0]} that runs the project's tests: "
                    f"{err.strerror}"
                ) from err
            self._live.add(process)
            return process


def _kill(process: subprocess.Popen[bytes]) -> None:
    """Kill the runner and whatever it started: a test may start processes of its own."""
    if process.poll() is not None:
        return
    try:
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGKILL)
        else:
            process.kill()
    except OSError:
        pass
