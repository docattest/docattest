"""Where attested code runs: the worker refuses a job it cannot read, the pool stops a job
whose worker writes something that is not a message, and the interpreter attested code runs
on is asked what it can import, or the failure to ask it is reported."""

import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
PACKAGE_ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
sys.path.insert(0, PACKAGE_ROOT)

from talk import HELLO, SURFACE, Talk, example, opened  # noqa: E402

from docattest_python.workers.docattest_worker.jobs import BlockJob, Settings  # noqa: E402
from docattest_python.workers.pool import Lost, WorkerPool  # noqa: E402
from docattest_python.workers.worker_process import WORKER, WorkerSpec  # noqa: E402

SETTINGS: Settings = {
    "path": [],
    "aliases": {},
    "snapshots": {"dir": None, "data": {}},
    "manifests": {},
    "texts": {},
    "preload": [],
    "no_fork": False,
}


def script(directory: str, text: str) -> str:
    """An executable that stands in for an interpreter."""
    path = os.path.join(directory, "python")
    with open(path, "w", encoding="utf-8") as handle:
        handle.write("#!/bin/sh\n" + text)
    os.chmod(path, os.stat(path).st_mode | stat.S_IXUSR)
    return path


class Worker(unittest.TestCase):
    def test_a_job_that_lacks_a_field_stops_the_worker(self):
        job = {
            "type": "example",
            "file": "<a>",
            "weak": False,
            "network": False,
            "before": [],
            "module": None,
        }
        lines = [json.dumps(SETTINGS), json.dumps({"job": job, "start": 0})]
        done = subprocess.run(
            [sys.executable, WORKER],
            input="\n".join(lines) + "\n",
            capture_output=True,
            text=True,
            timeout=60,
        )
        self.assertNotEqual(done.returncode, 0)
        self.assertIn('a job request: "job": "code" must be a string', done.stderr)


@unittest.skipUnless(os.name == "posix", "the stand-in interpreter is a shell script")
class Pool(unittest.TestCase):
    def test_a_line_that_is_not_a_message_ends_the_job(self):
        with tempfile.TemporaryDirectory() as directory:
            python = script(
                directory, 'echo \'{"t": "hello", "fork": true}\'\necho \'not a message\'\ncat >/dev/null\n'
            )
            pool = WorkerPool(1, WorkerSpec(python, directory, SETTINGS), 5000)
            job: BlockJob = {
                "type": "example",
                "file": "<a>",
                "code": "",
                "weak": False,
                "network": False,
                "before": [],
                "module": None,
            }
            result = pool.submit(job, 1).result(timeout=30)
            pool.close()
        (lost,) = result.items
        assert isinstance(lost, Lost)
        self.assertIn("the worker is broken: a line from the worker is not JSON", lost.message)


@unittest.skipUnless(os.name == "posix", "the stand-in interpreter is a shell script")
class Interpreter(unittest.TestCase):
    def compile(self, python: str, code: str):
        talk = Talk(
            HELLO,
            opened({"lib.py": SURFACE}),
            {"id": 3, "method": "compile", "params": {"examples": [example(code)]}},
            env={"DOCATTEST_PYTHON": python},
        )
        return talk.of(3)

    def test_another_interpreter_is_asked_what_it_can_import(self):
        with tempfile.TemporaryDirectory() as directory:
            python = script(directory, f'exec "{sys.executable}" "$@"\n')
            answer = self.compile(python, "import json\nimport no_such_module_anywhere\nassert json")
        self.assertEqual(
            answer["result"], [{"id": "e", "message": "No module named 'no_such_module_anywhere'", "line": 2}]
        )

    def test_starting_the_interpreter_is_not_counted_against_a_block(self):
        quick = example("assert True", id="quick")
        asleep = example("import time\ntime.sleep(1)", id="asleep")
        run = {"id": 3, "method": "run", "params": {"examples": [quick, asleep], "cases": []}}
        # Each block is forked from one worker, or (no fork) runs on a worker of its own.
        for no_fork in ("", "1"):
            with self.subTest(no_fork=no_fork), tempfile.TemporaryDirectory() as directory:
                python = script(directory, f'sleep 1\nexec "{sys.executable}" "$@"\n')
                talk = Talk(
                    HELLO,
                    opened({"lib.py": SURFACE}, workers=1, timeoutMs=300),
                    run,
                    env={"DOCATTEST_PYTHON": python, "DOCATTEST_PYTHON_NO_FORK": no_fork},
                )
                results = [(r["status"], r["message"]) for r in talk.result(3)]
                self.assertEqual(results[0][0], "pass")
                self.assertEqual(results[1], ("fail", "timed out after 300 ms"))

    def test_an_interpreter_that_cannot_be_asked_is_a_user_error(self):
        missing = os.path.join(tempfile.gettempdir(), "no-such-dir", "python")
        answer = self.compile(missing, "import json\nassert json")
        self.assertEqual(answer["error"]["kind"], "user")
        self.assertIn(
            f"cannot ask the Python interpreter {missing} which modules it can import",
            answer["error"]["message"],
        )

    def test_an_interpreter_that_cannot_be_started_is_a_user_error(self):
        missing = os.path.join(tempfile.gettempdir(), "no-such-dir", "python")
        talk = Talk(
            HELLO,
            opened({"lib.py": SURFACE}),
            {"id": 3, "method": "run", "params": {"examples": [example("assert True")], "cases": []}},
            env={"DOCATTEST_PYTHON": missing},
        )
        answer = talk.of(3)
        self.assertEqual(answer["error"]["kind"], "user")
        self.assertEqual(
            answer["error"]["message"],
            f"docattest: cannot start the Python interpreter {missing} that attested code runs on: "
            "No such file or directory",
        )


if __name__ == "__main__":
    unittest.main()


# Run in a process of its own: the guard changes the socket module for good.
GUARDED = """
import asyncio, socket, sys
sys.path.insert(0, sys.argv[1])
from docattest_worker.environment import NETWORK_OFF, guard_network
# Windows has no socketpair of its own: Python makes one from a loopback connection, and
# asyncio makes one for every event loop.
socket.socketpair = socket._fallback_socketpair
guard_network()
async def answer():
    return 42
print(asyncio.run(answer()))
try:
    socket.create_connection(("192.0.2.1", 80))
except OSError as refused:
    print(str(refused) == NETWORK_OFF)
pair = socket.socket()
try:
    pair.connect(("127.0.0.1", 9))
except OSError as refused:
    print(str(refused) == NETWORK_OFF)
"""


@unittest.skipUnless(hasattr(__import__("socket"), "_fallback_socketpair"), "Python 3.12 or later")
class Network(unittest.TestCase):
    def test_the_guard_lets_a_socketpair_through_and_refuses_the_rest(self):
        workers = os.path.join(PACKAGE_ROOT, "docattest_python", "workers")
        ran = subprocess.run(
            [sys.executable, "-c", GUARDED, workers], capture_output=True, text=True, timeout=60
        )
        self.assertEqual((ran.stdout.split(), ran.stderr), (["42", "True", "True"], ""))
