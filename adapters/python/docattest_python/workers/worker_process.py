"""One worker process, how it is started, and the thread that reads what it writes."""

from __future__ import annotations

import json
import os
import queue
import signal
import subprocess
import threading
from dataclasses import dataclass
from typing import IO

from ..errors import UserError
from .docattest_worker.fields import MalformedMessage
from .docattest_worker.jobs import JobRequest, Settings
from .docattest_worker.messages import Message, read_message

WORKER = os.path.join(os.path.dirname(os.path.abspath(__file__)), "worker.py")


class EndOfOutput:
    """The worker closed its end of the pipe: it exited, or was killed."""


#: What reading the worker gives: a message, a line that is not one, or the end.
Received = Message | MalformedMessage | EndOfOutput


@dataclass(frozen=True)
class WorkerSpec:
    """How a worker is started: the interpreter it runs on, the directory it runs in, and
    the settings it is sent before any job."""

    python: str
    cwd: str
    settings: Settings

    def start(self) -> WorkerProcess:
        return WorkerProcess(self)


class WorkerProcess:
    """One worker process and the thread that reads what it writes."""

    def __init__(self, spec: WorkerSpec) -> None:
        try:
            self.proc = subprocess.Popen(  # noqa: S603
                [spec.python, WORKER],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                cwd=spec.cwd if os.path.isdir(spec.cwd) else None,
                start_new_session=os.name == "posix",
            )
        except OSError as err:
            # The interpreter is the project's choice (DOCATTEST_PYTHON, a virtualenv), so
            # one that cannot be started is the project's mistake, not the adapter's.
            raise UserError(
                f"cannot start the Python interpreter {spec.python} that attested code runs on: "
                f"{err.strerror}"
            ) from err
        self._stdin: IO[bytes] = _pipe(self.proc.stdin)
        self._stdout: IO[bytes] = _pipe(self.proc.stdout)
        self.messages: queue.Queue[Received] = queue.Queue()
        self._held: Received | None = None
        #: Whether each job runs in a forked child; the worker's hello says.
        self.fork = True
        threading.Thread(target=self._read, daemon=True).start()
        self._write(json.dumps(spec.settings))

    def _read(self) -> None:
        try:
            for line in self._stdout:
                try:
                    self.messages.put(read_message(line))
                except MalformedMessage as err:
                    # Nothing but the worker writes here: a line that is not a message
                    # means the worker is broken, and nothing it says after is trusted.
                    self.messages.put(err)
                    return
        except (OSError, ValueError):
            pass
        self.messages.put(EndOfOutput())

    def _write(self, line: str) -> bool:
        try:
            self._stdin.write((line + "\n").encode("utf-8"))
            self._stdin.flush()
            return True
        except (OSError, ValueError):
            return False

    def send(self, request: JobRequest) -> bool:
        """Send a job; False when the worker cannot be reached."""
        return self._write(json.dumps(request))

    def greet(self, timeout: float) -> None:
        """Read the worker's hello. Anything else it says first is kept for ``next``."""
        first = self.next(timeout)
        if isinstance(first, dict) and first["t"] == "hello":
            self.fork = first["fork"]
        else:
            self._held = first

    def next(self, timeout: float) -> Received | None:
        """What the worker says next, or None when it says nothing for ``timeout`` seconds."""
        if self._held is not None:
            held, self._held = self._held, None
            return held
        try:
            return self.messages.get(timeout=timeout)
        except queue.Empty:
            return None

    def kill(self) -> None:
        if self.proc.poll() is None:
            try:
                if os.name == "posix":
                    os.killpg(self.proc.pid, signal.SIGKILL)
                else:
                    self.proc.kill()
            except OSError:
                pass
        self._reap()

    def stop(self) -> None:
        """Let an idle worker go: at the end of its input it exits by itself."""
        if self.proc.poll() is None:
            try:
                self._stdin.close()
            except OSError:
                pass
            try:
                self.proc.wait(timeout=1)
            except subprocess.TimeoutExpired:
                self.kill()
                return
        self._reap()

    def _reap(self) -> None:
        try:
            self.proc.wait(timeout=5)
        except (subprocess.TimeoutExpired, OSError):
            pass
        for stream in (self._stdin, self._stdout):
            try:
                stream.close()
            except (OSError, ValueError):
                pass


def _pipe(stream: IO[bytes] | None) -> IO[bytes]:
    """A pipe ``Popen`` was asked for, which it always opens."""
    assert stream is not None
    return stream


def ms(value: float) -> str:
    """Milliseconds as a message writes them: no ``.0`` on a whole number."""
    return str(int(value)) if float(value).is_integer() else str(value)
