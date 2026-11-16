"""The worker pool: a fixed number of worker processes, each running one job at a time.

A job is a list of items (one block; the facts of a page; the rows of a table). Every
phase of a job, preparing and then each item, gets the full time budget. When one overruns,
the worker's whole process group is killed, which stops the code wherever it is; the item
is recorded as timed out and the job goes on from the next item on a fresh worker.
"""

from __future__ import annotations

import queue
import sys
import threading
from concurrent.futures import Future
from dataclasses import dataclass, field
from typing import Literal

from .docattest_worker.fields import MalformedMessage
from .docattest_worker.jobs import Job, JobRequest
from .docattest_worker.messages import FailedMessage, Item
from .worker_process import EndOfOutput, WorkerProcess, WorkerSpec, ms

CLOSED = "the session was closed before this could run"
#: How long a new worker has to say hello, in seconds.
HELLO_TIMEOUT = 30
#: How long a worker that gave up on a job has to exit, in seconds.
EXIT_TIMEOUT = 1.0


@dataclass(frozen=True)
class TimedOut:
    """An item that overran the time budget."""


@dataclass(frozen=True)
class Lost:
    """An item that never ran: its worker went away before it, for this reason."""

    message: str


@dataclass(frozen=True)
class JobFailure:
    """Why a job could not even start: its surface or setup module did not load."""

    message: str
    timed_out: bool


ItemResult = Item | TimedOut | Lost


@dataclass
class JobResult:
    """A job's items in order, or why it could not start."""

    failure: JobFailure | None = None
    items: list[ItemResult] = field(default_factory=list[ItemResult])


@dataclass(frozen=True)
class _Done:
    kind: Literal["done", "timeout"]


@dataclass(frozen=True)
class _Stopped:
    """The job stopped before its end: ``failed`` when the worker gave up on it."""

    kind: Literal["failed", "crashed"]
    message: str


_End = _Done | _Stopped


@dataclass(frozen=True)
class _Attempt:
    """One attempt at a job on one worker: the items it got, how it ended, whether the job
    was prepared, and whether the worker can take another job."""

    items: list[Item]
    end: _End
    prepared: bool
    healthy: bool


class WorkerPool:
    def __init__(self, size: int, spec: WorkerSpec, timeout_ms: float) -> None:
        self.size = max(1, int(size))
        self.spec = spec
        self.timeout = max(timeout_ms, 1) / 1000.0
        self.timeout_ms = timeout_ms
        self.tasks: queue.Queue[tuple[Job, int, Future[JobResult]] | None] = queue.Queue()
        self.threads: list[threading.Thread] = []
        #: Jobs queued or running; a thread (and its worker) is added while it exceeds them.
        self.outstanding = 0
        self.lock = threading.Lock()
        self.closed = False
        self.live: set[WorkerProcess] = set()

    def submit(self, job: Job, count: int) -> Future[JobResult]:
        """Queue a job of ``count`` items; the future resolves to its ``JobResult``."""
        future: Future[JobResult] = Future()
        with self.lock:
            if self.closed:
                future.set_result(JobResult(JobFailure(CLOSED, False)))
                return future
            self.tasks.put((job, count, future))
            self.outstanding += 1
            if self.outstanding > len(self.threads) and len(self.threads) < self.size:
                thread = threading.Thread(target=self._serve, daemon=True)
                self.threads.append(thread)
                thread.start()
        return future

    def close(self) -> None:
        with self.lock:
            self.closed = True
            threads = list(self.threads)
        for worker in list(self.live):
            worker.kill()
        for _ in threads:
            self.tasks.put(None)
        for thread in threads:
            thread.join(timeout=5)

    # One thread per worker process: take a job, run it to the end, take the next.
    def _serve(self) -> None:
        worker: WorkerProcess | None = None
        while True:
            task = self.tasks.get()
            if task is None:
                break
            job, count, future = task
            try:
                result, worker = self._run(job, count, worker)
            except BaseException as err:  # noqa: BLE001 - the request waiting on it must hear
                with self.lock:
                    self.outstanding -= 1
                future.set_exception(err)
                continue
            with self.lock:
                self.outstanding -= 1
            future.set_result(result)
        if worker is not None:
            self.live.discard(worker)
            worker.stop()

    def _worker(self, worker: WorkerProcess | None) -> WorkerProcess:
        if worker is not None:
            return worker
        worker = self.spec.start()
        self.live.add(worker)
        worker.greet(HELLO_TIMEOUT)
        return worker

    def _discard(self, worker: WorkerProcess) -> None:
        self.live.discard(worker)
        worker.kill()

    def _run(
        self, job: Job, count: int, worker: WorkerProcess | None
    ) -> tuple[JobResult, WorkerProcess | None]:
        result = JobResult()
        while True:
            attempt, worker = self._try(job, len(result.items), worker)
            result.items.extend(attempt.items)
            end = attempt.end
            if isinstance(end, _Done) and end.kind == "done":
                return result, worker
            if (isinstance(end, _Stopped) and end.kind == "failed") or not attempt.prepared:
                self._give_up(result, end, count)
                return result, worker
            if len(result.items) >= count:
                return result, worker
            result.items.append(TimedOut() if isinstance(end, _Done) else Lost(end.message))
            if len(result.items) >= count:
                return result, worker

    def _try(
        self, job: Job, start: int, worker: WorkerProcess | None
    ) -> tuple[_Attempt, WorkerProcess | None]:
        """One attempt from item ``start``, on ``worker`` or a new one; also the worker to
        go on with, or None."""
        if self.closed:
            return _Attempt([], _Stopped("failed", CLOSED), False, True), worker
        worker = self._worker(worker)
        attempt = self._attempt(worker, job, start)
        if not attempt.healthy:
            self._discard(worker)
            return attempt, None
        if not worker.fork:
            # Without fork a worker runs one job in its own process and exits.
            self.live.discard(worker)
            worker.stop()
            return attempt, None
        return attempt, worker

    def _give_up(self, result: JobResult, end: _End, count: int) -> None:
        """The job cannot go on: before its first item the whole job failed; after it,
        every item left is lost, for the same reason."""
        timed_out = isinstance(end, _Done)
        message = f"timed out after {ms(self.timeout_ms)} ms" if isinstance(end, _Done) else end.message
        if not result.items:
            result.failure = JobFailure(message, timed_out)
            return
        while len(result.items) < count:
            result.items.append(Lost(message))

    def _attempt(self, worker: WorkerProcess, job: Job, start: int) -> _Attempt:
        """One attempt at a job on one worker, from item ``start``. Single-item jobs (a
        block, a session) need no preparing: their one phase is the item."""
        items: list[Item] = []
        prepared = job["type"] in ("example", "session")
        request: JobRequest = {"job": job, "start": start}
        if not worker.send(request):
            return _Attempt(items, _Stopped("crashed", "the worker could not be reached"), prepared, False)
        while True:
            message = worker.next(self.timeout)
            if message is None:
                return _Attempt(items, _Done("timeout"), prepared, False)
            if isinstance(message, MalformedMessage):
                return _Attempt(
                    items, _Stopped("crashed", f"the worker is broken: {message}"), prepared, False
                )
            if isinstance(message, EndOfOutput):
                code = worker.proc.poll()
                return _Attempt(
                    items, _Stopped("crashed", f"the worker exited with code {code}"), prepared, False
                )
            if message["t"] == "ready":
                prepared = True
            elif message["t"] == "failed":
                end, healthy = _failed(worker, message)
                return _Attempt(items, end, prepared, healthy)
            elif message["t"] == "exit":
                return _Attempt(
                    items, _exited(message["code"], len(items), _remaining(job, start)), prepared, True
                )
            elif message["t"] != "hello":
                items.append(message)


def _remaining(job: Job, start: int) -> int:
    """How many items a job has left from item ``start``."""
    if job["type"] == "facts":
        return len(job["facts"]) - start
    if job["type"] == "cases":
        return len(job["rows"]) - start
    return 1 - start


def _failed(worker: WorkerProcess, message: FailedMessage) -> tuple[_End, bool]:
    """A job the worker gave up on, and whether the worker is still fit for another."""
    if message["internal"]:
        sys.stderr.write(message["internal"] + "\n")
    # The child still has to exit; give it a moment, then move on.
    tail = worker.next(EXIT_TIMEOUT)
    healthy = isinstance(tail, dict) and tail["t"] == "exit"
    return _Stopped("failed", message["message"]), healthy


def _exited(code: int, received: int, expected: int) -> _End:
    """How a job ended when its child exited: done when the child sent every item."""
    if code == 0 and received >= expected:
        return _Done("done")
    if code < 0:
        return _Stopped("crashed", f"the worker was killed by signal {-code}")
    if code:
        return _Stopped("crashed", f"the worker exited with code {code} before it finished")
    return _Stopped("crashed", "the worker stopped before it finished")
