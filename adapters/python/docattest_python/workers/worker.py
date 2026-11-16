"""The worker: where attested code runs, apart from the adapter.

Started by the adapter as ``python worker.py`` (standard library only, so it also runs on
another interpreter, such as the project's virtualenv). It reads one JSON line of settings,
then one JSON line per job, and writes JSON lines back on the pipe that was its stdout.
Descriptor 1 is pointed at stderr before anything else, so nothing attested code prints can
reach that pipe.

Each job runs in a forked child of this process: a fresh ``__main__``, fresh imports of the
project, no state shared with the job before it. Where ``os.fork`` is missing the worker
runs one job and exits, and the adapter starts another.
"""

from __future__ import annotations

import json
import os
import sys
from collections.abc import Iterator
from typing import BinaryIO

# Started by its path, the worker imports its parts from beside it. Its directory goes
# first even where the interpreter leaves it out (PYTHONSAFEPATH); main() takes it off.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from docattest_worker.cases import run_cases  # noqa: E402
from docattest_worker.channel import Channel  # noqa: E402
from docattest_worker.environment import Environment  # noqa: E402
from docattest_worker.examples import run_example  # noqa: E402
from docattest_worker.facts import run_facts  # noqa: E402
from docattest_worker.jobs import JobRequest, Settings, read_request, read_settings  # noqa: E402
from docattest_worker.observables import run_observables  # noqa: E402

#: How a forked child that failed outside its job exits (EX_SOFTWARE).
INTERNAL_FAILURE = 70


def main() -> None:
    out = os.dup(1)
    os.dup2(2, 1)
    channel = Channel(out)
    own = os.path.dirname(os.path.abspath(__file__))
    sys.path[:] = [p for p in sys.path if os.path.abspath(p or ".") != own]
    stdin = sys.stdin.buffer
    settings = read_settings(json.loads(stdin.readline()))
    preload(settings["preload"])
    fork = hasattr(os, "fork") and not settings["no_fork"]
    channel.post({"t": "hello", "fork": fork})
    for request in requests(stdin):
        if not fork:
            run_job(request, settings, channel)
            channel.post({"t": "exit", "code": 0})
            os._exit(0)
        run_forked(request, settings, channel)


def preload(names: list[str]) -> None:
    """Import, once and before any job is forked, modules the jobs are expected to need
    (``doctest`` when the pages hold sessions); one that is missing is left to the jobs."""
    for name in names:
        try:
            __import__(name)
        except ImportError:
            pass


def requests(stdin: BinaryIO) -> Iterator[JobRequest]:
    """Each job request, one JSON line each, until the adapter closes the pipe. A request
    that is not one is the adapter's fault: the worker stops with the reason."""
    while True:
        line = stdin.readline()
        if not line:
            return
        if line.strip():
            yield read_request(json.loads(line))


def run_job(request: JobRequest, settings: Settings, channel: Channel) -> None:
    try:
        _dispatch(request, Environment(settings), channel)
    except BaseException as err:  # noqa: BLE001 - the adapter must hear of it, whatever it is
        import traceback

        detail = "".join(traceback.format_exception(type(err), err, err.__traceback__))
        channel.post(
            {
                "t": "failed",
                "message": "the worker failed: " + detail.strip().split("\n")[-1],
                "internal": detail,
            }
        )


def _dispatch(request: JobRequest, env: Environment, channel: Channel) -> None:
    job, start = request["job"], request["start"]
    if job["type"] == "facts":
        run_facts(job, env, channel, start)
    elif job["type"] == "cases":
        run_cases(job, env, channel, start)
    elif job["type"] == "observables":
        run_observables(job, env, channel)
    elif job["type"] == "example":
        run_example(job, env, channel)
    else:
        # Imported only when a session is run: doctest is slow to import.
        from docattest_worker.sessions import run_session

        run_session(job, env, channel)


def run_forked(request: JobRequest, settings: Settings, channel: Channel) -> None:
    """Run one job in a forked child, and say how the child exited."""
    pid = os.fork()
    if pid == 0:
        code = 0
        try:
            run_job(request, settings, channel)
        except BaseException:  # noqa: BLE001
            code = INTERNAL_FAILURE
        finally:
            os._exit(code)
    _, status = os.waitpid(pid, 0)
    channel.post({"t": "exit", "code": os.waitstatus_to_exitcode(status)})


if __name__ == "__main__":
    main()
